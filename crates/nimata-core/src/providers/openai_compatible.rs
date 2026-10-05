//! Servers that speak OpenAI's Chat Completions API: local runners such as
//! Ollama, LM Studio, llama.cpp, and vLLM, and hosted services such as xAI
//! or Qwen's compatible endpoints. The key is optional.

use futures_util::future::BoxFuture;
use reqwest::Client;
use serde_json::{Value, json};

use super::{
    Completion, ModelInfo, ModelProvider, ModelRequest, ProviderError, ProviderErrorKind,
    ResponseStream, Role, StreamEvent, Usage, header, network_error, sse_stream, status_error,
};

pub struct OpenAiCompatible {
    client: Client,
    base_url: String,
    api_key: Option<String>,
    /// The connection's name, used in messages ("Ollama is overloaded…").
    name: String,
}

impl OpenAiCompatible {
    pub fn new(client: Client, base_url: &str, api_key: Option<String>, name: &str) -> Self {
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.filter(|k| !k.trim().is_empty()),
            name: name.to_string(),
        }
    }

    fn authorized(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.api_key {
            Some(key) => request.bearer_auth(key),
            None => request,
        }
    }

    pub fn request_body(request: &ModelRequest, with_usage: bool) -> Value {
        let mut messages = vec![json!({ "role": "system", "content": request.instructions })];
        messages.extend(request.messages.iter().map(|m| {
            let role = match m.role {
                Role::User => "user",
                Role::Assistant => "assistant",
            };
            json!({ "role": role, "content": m.text })
        }));
        let mut body = json!({ "model": request.model, "messages": messages, "stream": true });
        if with_usage {
            body["stream_options"] = json!({ "include_usage": true });
        }
        body
    }
}

/// Reads one streamed chunk. The final `[DONE]` ends the reply; usage and the
/// finish reason may arrive in earlier chunks.
#[derive(Default)]
pub struct ChunkParser {
    completion: Completion,
    name: String,
}

impl ChunkParser {
    pub fn new(name: &str) -> Self {
        Self {
            completion: Completion::default(),
            name: name.to_string(),
        }
    }

    pub fn parse(&mut self, data: &str) -> Option<Result<StreamEvent, ProviderError>> {
        if data.trim() == "[DONE]" {
            return Some(Ok(StreamEvent::Done(std::mem::take(&mut self.completion))));
        }
        let chunk: Value = serde_json::from_str(data).ok()?;
        if let Some(error) = chunk.get("error") {
            let detail = error
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| error.as_str().map(str::to_string));
            return Some(Err(ProviderError::new(
                ProviderErrorKind::Server,
                format!("{} reported an error while replying.", self.name),
            )
            .with_detail(detail)));
        }
        let text = |pointer: &str| {
            chunk
                .pointer(pointer)
                .and_then(Value::as_str)
                .map(str::to_string)
        };
        if self.completion.response_id.is_none() {
            self.completion.response_id = text("/id");
        }
        if let Some(model) = text("/model") {
            self.completion.model = Some(model);
        }
        if let (Some(input_tokens), Some(output_tokens)) = (
            chunk
                .pointer("/usage/prompt_tokens")
                .and_then(Value::as_u64),
            chunk
                .pointer("/usage/completion_tokens")
                .and_then(Value::as_u64),
        ) {
            self.completion.usage = Some(Usage {
                input_tokens,
                output_tokens,
            });
        }
        if let Some(reason) = text("/choices/0/finish_reason") {
            self.completion.incomplete_reason = (reason != "stop").then_some(reason);
        }
        let delta = text("/choices/0/delta/content").filter(|d| !d.is_empty())?;
        Some(Ok(StreamEvent::Delta(delta)))
    }
}

impl ModelProvider for OpenAiCompatible {
    fn models(&self) -> BoxFuture<'_, Result<Vec<ModelInfo>, ProviderError>> {
        Box::pin(async move {
            let response = self
                .authorized(self.client.get(format!("{}/models", self.base_url)))
                .send()
                .await
                .map_err(|e| network_error(&self.name, &e))?;
            let status = response.status();
            let request_id = header(&response, "x-request-id");
            let body = response
                .text()
                .await
                .map_err(|e| network_error(&self.name, &e))?;
            if !status.is_success() {
                return Err(status_error(&self.name, status.as_u16(), &body, None)
                    .with_request_id(request_id));
            }
            let parsed: Value = serde_json::from_str(&body).map_err(|e| {
                ProviderError::new(
                    ProviderErrorKind::Server,
                    format!(
                        "{} sent an unreadable model list. Check the endpoint address.",
                        self.name
                    ),
                )
                .with_detail(Some(e.to_string()))
            })?;
            let mut models: Vec<ModelInfo> = parsed
                .get("data")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|m| m.get("id").and_then(Value::as_str))
                .map(|id| ModelInfo { id: id.to_string() })
                .collect();
            models.sort_by(|a, b| a.id.cmp(&b.id));
            Ok(models)
        })
    }

    fn respond(
        &self,
        request: ModelRequest,
    ) -> BoxFuture<'_, Result<ResponseStream, ProviderError>> {
        Box::pin(async move {
            let mut with_usage = true;
            loop {
                let response = self
                    .authorized(
                        self.client
                            .post(format!("{}/chat/completions", self.base_url)),
                    )
                    .json(&Self::request_body(&request, with_usage))
                    .send()
                    .await
                    .map_err(|e| network_error(&self.name, &e))?;
                let status = response.status();
                let request_id = header(&response, "x-request-id");
                if !status.is_success() {
                    let body = response.text().await.unwrap_or_default();
                    // Some servers reject the usage option; ask again without it.
                    if with_usage && status.as_u16() == 400 && body.contains("stream_options") {
                        with_usage = false;
                        continue;
                    }
                    return Err(status_error(
                        &self.name,
                        status.as_u16(),
                        &body,
                        Some(&request.model),
                    )
                    .with_request_id(request_id));
                }
                let mut parser = ChunkParser::new(&self.name);
                let events = sse_stream(response, self.name.clone(), move |event| {
                    parser.parse(&event.data)
                });
                return Ok(ResponseStream { request_id, events });
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::Message;

    #[test]
    fn instructions_become_the_system_message() {
        let body = OpenAiCompatible::request_body(
            &ModelRequest {
                model: "qwen3".into(),
                instructions: "You are Qwen.".into(),
                messages: vec![Message {
                    role: Role::User,
                    text: "Thanos:\nHi".into(),
                }],
            },
            true,
        );
        assert_eq!(
            body["messages"][0],
            json!({ "role": "system", "content": "You are Qwen." })
        );
        assert_eq!(body["messages"][1]["role"], "user");
        assert_eq!(body["stream_options"]["include_usage"], true);
        assert!(
            OpenAiCompatible::request_body(
                &ModelRequest {
                    model: "m".into(),
                    instructions: String::new(),
                    messages: vec![]
                },
                false
            )
            .get("stream_options")
            .is_none()
        );
    }

    #[test]
    fn chunks_build_the_reply_usage_and_finish_reason() {
        let mut p = ChunkParser::new("Ollama");
        assert_eq!(
            p.parse(r#"{"id":"chatcmpl-1","model":"qwen3:8b","choices":[{"index":0,"delta":{"role":"assistant","content":""},"finish_reason":null}]}"#),
            None
        );
        assert_eq!(
            p.parse(r#"{"id":"chatcmpl-1","model":"qwen3:8b","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#),
            Some(Ok(StreamEvent::Delta("Hello".into())))
        );
        assert_eq!(
            p.parse(r#"{"id":"chatcmpl-1","model":"qwen3:8b","choices":[{"index":0,"delta":{},"finish_reason":"length"}]}"#),
            None
        );
        assert_eq!(
            p.parse(r#"{"id":"chatcmpl-1","choices":[],"usage":{"prompt_tokens":20,"completion_tokens":3,"total_tokens":23}}"#),
            None
        );
        assert_eq!(
            p.parse("[DONE]"),
            Some(Ok(StreamEvent::Done(Completion {
                response_id: Some("chatcmpl-1".into()),
                model: Some("qwen3:8b".into()),
                usage: Some(Usage {
                    input_tokens: 20,
                    output_tokens: 3
                }),
                incomplete_reason: Some("length".into()),
            })))
        );
    }

    #[test]
    fn error_chunks_name_the_server() {
        let mut p = ChunkParser::new("LM Studio");
        let Some(Err(e)) = p.parse(r#"{"error":{"message":"model not loaded"}}"#) else {
            panic!()
        };
        assert_eq!(e.message, "LM Studio reported an error while replying.");
        assert_eq!(e.detail.as_deref(), Some("model not loaded"));
        let Some(Err(e)) = p.parse(r#"{"error":"out of memory"}"#) else {
            panic!()
        };
        assert_eq!(e.detail.as_deref(), Some("out of memory"));
    }
}
