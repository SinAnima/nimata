//! Anthropic adapter, using the Messages API (`POST /messages`) with
//! streaming, and `GET /models` for discovery.

use futures_util::future::BoxFuture;
use reqwest::Client;
use serde_json::{Value, json};

use super::{
    Completion, FileKind, Message, ModelInfo, ModelProvider, ModelRequest, ProviderError,
    ProviderErrorKind, ResponseStream, Role, StreamEvent, Usage, header, network_error, sse_stream,
    status_error,
};

pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com/v1";
const PROVIDER: &str = "Anthropic";
const API_VERSION: &str = "2023-06-01";

/// Room for a long reply on current models. Older models allow less; a
/// request they reject for its size is retried once with
/// [`SMALL_MAX_TOKENS`].
const MAX_TOKENS: u32 = 32_000;
const SMALL_MAX_TOKENS: u32 = 8_192;

pub struct Anthropic {
    client: Client,
    base_url: String,
    api_key: String,
}

impl Anthropic {
    pub fn new(client: Client, base_url: &str, api_key: String) -> Self {
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
        }
    }

    fn get(&self, path: &str) -> reqwest::RequestBuilder {
        self.client
            .get(format!("{}{path}", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
    }

    /// The conversation must start with the user and, on current models, end
    /// with the user (they do not continue a reply of their own). Bridge
    /// either end with a short user turn when the thread starts or ends with
    /// the responding model's own post.
    pub fn conversation(messages: &[Message]) -> Vec<Message> {
        let mut out = Vec::with_capacity(messages.len() + 2);
        if messages.first().is_none_or(|m| m.role == Role::Assistant) {
            out.push(Message {
                role: Role::User,
                text: "(The discussion so far follows.)".into(),
                files: vec![],
            });
        }
        out.extend(messages.iter().cloned());
        if out.last().is_some_and(|m| m.role == Role::Assistant) {
            out.push(Message {
                role: Role::User,
                text: "(Please follow up on your last post above.)".into(),
                files: vec![],
            });
        }
        out
    }

    pub fn request_body(request: &ModelRequest, max_tokens: u32) -> Value {
        let messages: Vec<Value> = Self::conversation(&request.messages)
            .iter()
            .map(|m| {
                let role = match m.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                };
                if m.files.is_empty() {
                    return json!({ "role": role, "content": m.text });
                }
                // Files first, then the text that refers to them.
                let mut content: Vec<Value> = m
                    .files
                    .iter()
                    .map(|f| match f.kind {
                        FileKind::Image => json!({
                            "type": "image",
                            "source": { "type": "base64", "media_type": f.media_type, "data": f.base64() },
                        }),
                        FileKind::Pdf => json!({
                            "type": "document",
                            "source": { "type": "base64", "media_type": "application/pdf", "data": f.base64() },
                            "title": f.filename,
                        }),
                    })
                    .collect();
                content.push(json!({ "type": "text", "text": m.text }));
                json!({ "role": role, "content": content })
            })
            .collect();
        json!({
            "model": request.model,
            "max_tokens": max_tokens,
            "system": request.instructions,
            "messages": messages,
            "stream": true,
        })
    }
}

/// Reads one streamed event, keeping what later events need: the message
/// ID, model, and token counts arrive in different events.
#[derive(Default)]
pub struct EventParser {
    completion: Completion,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    stop_reason: Option<String>,
}

impl EventParser {
    pub fn parse(&mut self, data: &str) -> Option<Result<StreamEvent, ProviderError>> {
        let event: Value = serde_json::from_str(data).ok()?;
        let text = |v: &Value, pointer: &str| {
            v.pointer(pointer)
                .and_then(Value::as_str)
                .map(str::to_string)
        };
        match event.get("type")?.as_str()? {
            "message_start" => {
                self.completion.response_id = text(&event, "/message/id");
                self.completion.model = text(&event, "/message/model");
                self.input_tokens = event
                    .pointer("/message/usage/input_tokens")
                    .and_then(Value::as_u64);
                None
            }
            "content_block_delta" => match event.pointer("/delta/type")?.as_str()? {
                "text_delta" => Some(Ok(StreamEvent::Delta(text(&event, "/delta/text")?))),
                // Thinking and other block types are not part of the reply.
                _ => None,
            },
            "message_delta" => {
                if let Some(reason) = text(&event, "/delta/stop_reason") {
                    self.stop_reason = Some(reason);
                }
                if let Some(tokens) = event
                    .pointer("/usage/output_tokens")
                    .and_then(Value::as_u64)
                {
                    self.output_tokens = Some(tokens);
                }
                None
            }
            "message_stop" => {
                if self.stop_reason.as_deref() == Some("refusal") {
                    return Some(Err(ProviderError::new(
                        ProviderErrorKind::Refused,
                        "The model declined to answer this.",
                    )));
                }
                let mut done = std::mem::take(&mut self.completion);
                done.usage = match (self.input_tokens, self.output_tokens) {
                    (Some(input_tokens), Some(output_tokens)) => Some(Usage {
                        input_tokens,
                        output_tokens,
                    }),
                    _ => None,
                };
                done.incomplete_reason = self
                    .stop_reason
                    .clone()
                    .filter(|r| r != "end_turn" && r != "stop_sequence");
                Some(Ok(StreamEvent::Done(done)))
            }
            "error" => {
                let kind = text(&event, "/error/type").unwrap_or_default();
                let message = text(&event, "/error/message");
                let (kind, summary) = match kind.as_str() {
                    "overloaded_error" => (
                        ProviderErrorKind::Server,
                        "Anthropic is overloaded right now. Retry in a moment.",
                    ),
                    "rate_limit_error" => (
                        ProviderErrorKind::RateLimit,
                        "Anthropic is limiting requests right now. Wait a moment, then retry.",
                    ),
                    _ => (
                        ProviderErrorKind::Server,
                        "Anthropic reported an error while replying.",
                    ),
                };
                let detail =
                    message.or_else(|| Some(event.to_string().chars().take(500).collect()));
                Some(Err(ProviderError::new(kind, summary).with_detail(detail)))
            }
            _ => None,
        }
    }
}

impl ModelProvider for Anthropic {
    fn models(&self) -> BoxFuture<'_, Result<Vec<ModelInfo>, ProviderError>> {
        Box::pin(async move {
            let mut models = Vec::new();
            let mut after: Option<String> = None;
            for _ in 0..10 {
                let path = match &after {
                    Some(after) => format!("/models?limit=1000&after_id={after}"),
                    None => "/models?limit=1000".to_string(),
                };
                let response = self
                    .get(&path)
                    .send()
                    .await
                    .map_err(|e| network_error(PROVIDER, &e))?;
                let status = response.status();
                let request_id = header(&response, "request-id");
                let body = response
                    .text()
                    .await
                    .map_err(|e| network_error(PROVIDER, &e))?;
                if !status.is_success() {
                    return Err(status_error(PROVIDER, status.as_u16(), &body, None)
                        .with_request_id(request_id));
                }
                let page: Value = serde_json::from_str(&body).map_err(|e| {
                    ProviderError::new(
                        ProviderErrorKind::Server,
                        "Anthropic sent an unreadable model list.",
                    )
                    .with_detail(Some(e.to_string()))
                })?;
                models.extend(
                    page.get("data")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(|m| m.get("id").and_then(Value::as_str))
                        .map(|id| ModelInfo { id: id.to_string() }),
                );
                if page.get("has_more").and_then(Value::as_bool) != Some(true) {
                    break;
                }
                after = page
                    .get("last_id")
                    .and_then(Value::as_str)
                    .map(str::to_string);
            }
            Ok(models)
        })
    }

    fn respond(
        &self,
        request: ModelRequest,
    ) -> BoxFuture<'_, Result<ResponseStream, ProviderError>> {
        Box::pin(async move {
            let mut max_tokens = MAX_TOKENS;
            loop {
                let response = self
                    .client
                    .post(format!("{}/messages", self.base_url))
                    .header("x-api-key", &self.api_key)
                    .header("anthropic-version", API_VERSION)
                    .json(&Self::request_body(&request, max_tokens))
                    .send()
                    .await
                    .map_err(|e| network_error(PROVIDER, &e))?;
                let status = response.status();
                let request_id = header(&response, "request-id");
                if !status.is_success() {
                    let body = response.text().await.unwrap_or_default();
                    if status.as_u16() == 400
                        && body.contains("max_tokens")
                        && max_tokens != SMALL_MAX_TOKENS
                    {
                        max_tokens = SMALL_MAX_TOKENS;
                        continue;
                    }
                    return Err(status_error(
                        PROVIDER,
                        status.as_u16(),
                        &body,
                        Some(&request.model),
                    )
                    .with_request_id(request_id));
                }
                let mut parser = EventParser::default();
                let events = sse_stream(response, PROVIDER.to_string(), move |event| {
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

    fn message(role: Role, text: &str) -> Message {
        Message {
            role,
            text: text.into(),
            files: vec![],
        }
    }

    #[test]
    fn the_conversation_starts_and_ends_with_the_user() {
        let plain = vec![
            message(Role::User, "Q"),
            message(Role::Assistant, "A"),
            message(Role::User, "Q2"),
        ];
        assert_eq!(Anthropic::conversation(&plain), plain);

        let own_post_last = vec![message(Role::User, "Q"), message(Role::Assistant, "A")];
        let fixed = Anthropic::conversation(&own_post_last);
        assert_eq!(fixed.len(), 3);
        assert_eq!(fixed.last().unwrap().role, Role::User);

        let own_post_first = vec![message(Role::Assistant, "A"), message(Role::User, "Q")];
        assert_eq!(Anthropic::conversation(&own_post_first)[0].role, Role::User);
    }

    #[test]
    fn requests_carry_the_instructions_as_the_system_prompt() {
        let body = Anthropic::request_body(
            &ModelRequest {
                model: "claude-opus-5-5".into(),
                instructions: "You are Claude.".into(),
                messages: vec![message(Role::User, "Thanos:\nQuestion")],
            },
            MAX_TOKENS,
        );
        assert_eq!(body["system"], "You are Claude.");
        assert_eq!(body["max_tokens"], 32_000);
        assert_eq!(body["stream"], true);
        assert_eq!(
            body["messages"][0],
            json!({ "role": "user", "content": "Thanos:\nQuestion" })
        );
    }

    #[test]
    fn stream_events_build_the_reply_and_its_usage() {
        let mut p = EventParser::default();
        assert_eq!(
            p.parse(r#"{"type":"message_start","message":{"id":"msg_1","model":"claude-opus-5-5","usage":{"input_tokens":30,"output_tokens":1}}}"#),
            None
        );
        assert_eq!(p.parse(r#"{"type":"ping"}"#), None);
        assert_eq!(
            p.parse(r#"{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":""}}"#),
            None,
            "thinking is not part of the reply"
        );
        assert_eq!(
            p.parse(r#"{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"Hi"}}"#),
            Some(Ok(StreamEvent::Delta("Hi".into())))
        );
        assert_eq!(p.parse(r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":9}}"#), None);
        assert_eq!(
            p.parse(r#"{"type":"message_stop"}"#),
            Some(Ok(StreamEvent::Done(Completion {
                response_id: Some("msg_1".into()),
                model: Some("claude-opus-5-5".into()),
                usage: Some(Usage {
                    input_tokens: 30,
                    output_tokens: 9
                }),
                incomplete_reason: None,
            })))
        );
    }

    #[test]
    fn length_limits_refusals_and_errors_are_reported() {
        let mut p = EventParser::default();
        p.parse(r#"{"type":"message_delta","delta":{"stop_reason":"max_tokens"},"usage":{"output_tokens":5}}"#);
        let Some(Ok(StreamEvent::Done(done))) = p.parse(r#"{"type":"message_stop"}"#) else {
            panic!()
        };
        assert_eq!(done.incomplete_reason.as_deref(), Some("max_tokens"));

        let mut p = EventParser::default();
        p.parse(r#"{"type":"message_delta","delta":{"stop_reason":"refusal"},"usage":{"output_tokens":5}}"#);
        let Some(Err(e)) = p.parse(r#"{"type":"message_stop"}"#) else {
            panic!()
        };
        assert_eq!(e.kind, ProviderErrorKind::Refused);

        let mut p = EventParser::default();
        let Some(Err(e)) = p.parse(
            r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#,
        ) else {
            panic!()
        };
        assert!(e.message.contains("overloaded"));
        assert_eq!(e.detail.as_deref(), Some("Overloaded"));
    }

    #[test]
    fn files_are_sent_as_blocks_before_the_text() {
        use crate::providers::FilePart;
        let request = ModelRequest {
            model: "claude-opus-5-5".into(),
            instructions: "Be brief.".into(),
            messages: vec![Message {
                role: Role::User,
                text: "Thanos:\nSee attached.".into(),
                files: vec![
                    FilePart {
                        filename: "a.png".into(),
                        media_type: "image/png".into(),
                        kind: FileKind::Image,
                        data: b"png".to_vec(),
                    },
                    FilePart {
                        filename: "p.pdf".into(),
                        media_type: "application/pdf".into(),
                        kind: FileKind::Pdf,
                        data: b"%PDF".to_vec(),
                    },
                ],
            }],
        };
        let body = Anthropic::request_body(&request, 1000);
        assert_eq!(
            body["messages"][0]["content"],
            json!([
                { "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": "cG5n" } },
                { "type": "document", "source": { "type": "base64", "media_type": "application/pdf", "data": "JVBERg==" }, "title": "p.pdf" },
                { "type": "text", "text": "Thanos:\nSee attached." },
            ])
        );
    }
}
