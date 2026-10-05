//! OpenAI adapter, using the Responses API (`POST /responses`) with
//! streaming, and `GET /models` for discovery.
//!
//! Requests are sent with `store: false`, so OpenAI does not keep the
//! conversation on its side: the discussion stays Nimata's.

use futures_util::future::BoxFuture;
use reqwest::{Client, StatusCode};
use serde_json::{Value, json};

use super::{
    Completion, ModelInfo, ModelProvider, ModelRequest, ProviderError, ProviderErrorKind,
    ResponseStream, Role, StreamEvent, Usage, header, network_error, sse_stream, status_error,
};

pub const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

pub struct OpenAi {
    client: Client,
    base_url: String,
    api_key: String,
}

impl OpenAi {
    pub fn new(client: Client, base_url: &str, api_key: String) -> Self {
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
        }
    }

    fn request_body(request: &ModelRequest) -> Value {
        let input: Vec<Value> = request
            .messages
            .iter()
            .map(|m| {
                let role = match m.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                };
                json!({ "role": role, "content": m.text })
            })
            .collect();
        json!({
            "model": request.model,
            "instructions": request.instructions,
            "input": input,
            "stream": true,
            "store": false,
        })
    }
}

/// Models that can write text replies. `/models` also lists embedding,
/// speech, image, and moderation models, which cannot take part.
pub fn is_text_model(id: &str) -> bool {
    let id = id.to_ascii_lowercase();
    let text_family = ["gpt-", "chatgpt-", "o1", "o3", "o4", "o5"]
        .iter()
        .any(|prefix| id.starts_with(prefix));
    let not_text = [
        "embedding",
        "tts",
        "whisper",
        "dall-e",
        "audio",
        "realtime",
        "transcribe",
        "image",
        "moderation",
        "search",
        "computer-use",
    ];
    text_family && !not_text.iter().any(|word| id.contains(word))
}

const PROVIDER: &str = "OpenAI";

/// Turns an HTTP error from OpenAI into a message the user can act on.
pub fn http_error(status: StatusCode, body: &str, model: Option<&str>) -> ProviderError {
    status_error(PROVIDER, status.as_u16(), body, model)
}

fn usage(response: &Value) -> Option<Usage> {
    let usage = response.get("usage")?;
    Some(Usage {
        input_tokens: usage.get("input_tokens")?.as_u64()?,
        output_tokens: usage.get("output_tokens")?.as_u64()?,
    })
}

fn completion(response: &Value) -> Completion {
    let text = |key: &str| {
        response
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_string)
    };
    Completion {
        response_id: text("id"),
        model: text("model"),
        usage: usage(response),
        incomplete_reason: response
            .pointer("/incomplete_details/reason")
            .and_then(Value::as_str)
            .map(str::to_string),
    }
}

/// An error reported inside the stream. Its message and code are read from
/// `error`; if there is no message, the raw event is kept as the detail so
/// the provider's reason is never lost.
fn stream_error(error: Option<&Value>, fallback: &str, event: &Value) -> ProviderError {
    let text = |key: &str| error.and_then(|e| e.get(key)).and_then(Value::as_str);
    let code = text("code").or_else(|| text("type")).unwrap_or_default();
    let (kind, message) = match code {
        "rate_limit_exceeded" => (
            ProviderErrorKind::RateLimit,
            "OpenAI is limiting requests right now. Wait a moment, then retry.",
        ),
        "insufficient_quota" => (
            ProviderErrorKind::Quota,
            "The OpenAI account has run out of credit or reached its spending limit.",
        ),
        "model_not_found" => (
            ProviderErrorKind::ModelUnavailable,
            "This model is not available to this API key.",
        ),
        _ => (ProviderErrorKind::Server, fallback),
    };
    let detail = match text("message") {
        Some(message) => Some(match code {
            "" => message.to_string(),
            code => format!("{message} [{code}]"),
        }),
        None => {
            let raw = event.to_string();
            Some(raw.chars().take(500).collect())
        }
    };
    ProviderError::new(kind, message).with_detail(detail)
}

/// Interprets one streamed event. `None` means the event is not relevant
/// to Nimata (OpenAI sends many bookkeeping events).
pub fn parse_event(data: &str) -> Option<Result<StreamEvent, ProviderError>> {
    let event: Value = serde_json::from_str(data).ok()?;
    match event.get("type")?.as_str()? {
        "response.output_text.delta" => {
            let delta = event.get("delta")?.as_str()?;
            Some(Ok(StreamEvent::Delta(delta.to_string())))
        }
        "response.completed" | "response.incomplete" => {
            Some(Ok(StreamEvent::Done(completion(event.get("response")?))))
        }
        "response.failed" => {
            let error = event.pointer("/response/error");
            Some(Err(stream_error(
                error,
                "OpenAI could not finish the reply.",
                &event,
            )))
        }
        "error" => {
            // The message and code may sit on the event itself or inside an
            // `error` object, depending on the API version; read both.
            let error = event
                .get("error")
                .filter(|e| e.is_object())
                .or(Some(&event));
            Some(Err(stream_error(
                error,
                "OpenAI reported an error while replying.",
                &event,
            )))
        }
        _ => None,
    }
}

impl ModelProvider for OpenAi {
    fn models(&self) -> BoxFuture<'_, Result<Vec<ModelInfo>, ProviderError>> {
        Box::pin(async move {
            let response = self
                .client
                .get(format!("{}/models", self.base_url))
                .bearer_auth(&self.api_key)
                .send()
                .await
                .map_err(|e| network_error(PROVIDER, &e))?;
            let status = response.status();
            let request_id = header(&response, "x-request-id");
            let body = response
                .text()
                .await
                .map_err(|e| network_error(PROVIDER, &e))?;
            if !status.is_success() {
                return Err(http_error(status, &body, None).with_request_id(request_id));
            }
            let parsed: Value = serde_json::from_str(&body).map_err(|e| {
                ProviderError::new(
                    ProviderErrorKind::Server,
                    "OpenAI sent an unreadable model list.",
                )
                .with_detail(Some(e.to_string()))
            })?;
            let mut models: Vec<ModelInfo> = parsed
                .get("data")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|m| m.get("id").and_then(Value::as_str))
                .filter(|id| is_text_model(id))
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
            let response = self
                .client
                .post(format!("{}/responses", self.base_url))
                .bearer_auth(&self.api_key)
                .json(&Self::request_body(&request))
                .send()
                .await
                .map_err(|e| network_error(PROVIDER, &e))?;
            let status = response.status();
            let request_id = header(&response, "x-request-id");
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                return Err(
                    http_error(status, &body, Some(&request.model)).with_request_id(request_id)
                );
            }

            let events = sse_stream(response, PROVIDER.to_string(), |event| {
                parse_event(&event.data)
            });
            Ok(ResponseStream { request_id, events })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::Message;

    #[test]
    fn requests_ask_openai_not_to_store_the_conversation() {
        let body = OpenAi::request_body(&ModelRequest {
            model: "gpt-5.6".into(),
            instructions: "Be brief.".into(),
            messages: vec![
                Message {
                    role: Role::User,
                    text: "Thanos:\nQuestion".into(),
                },
                Message {
                    role: Role::Assistant,
                    text: "Answer".into(),
                },
            ],
        });
        assert_eq!(body["store"], false);
        assert_eq!(body["stream"], true);
        assert_eq!(body["instructions"], "Be brief.");
        assert_eq!(
            body["input"][0],
            json!({ "role": "user", "content": "Thanos:\nQuestion" })
        );
        assert_eq!(body["input"][1]["role"], "assistant");
    }

    #[test]
    fn only_text_models_are_offered() {
        for id in ["gpt-5.6", "gpt-5.6-mini", "o4-mini", "chatgpt-4o-latest"] {
            assert!(is_text_model(id), "{id}");
        }
        for id in [
            "text-embedding-3-large",
            "gpt-4o-mini-tts",
            "whisper-1",
            "dall-e-3",
            "gpt-4o-realtime-preview",
            "gpt-image-1",
            "omni-moderation-latest",
            "gpt-4o-transcribe",
            "davinci-002",
        ] {
            assert!(!is_text_model(id), "{id}");
        }
    }

    #[test]
    fn http_errors_become_actionable_messages() {
        let quota = http_error(
            StatusCode::TOO_MANY_REQUESTS,
            r#"{"error":{"message":"You exceeded your current quota","code":"insufficient_quota"}}"#,
            Some("gpt-5.6"),
        );
        assert_eq!(quota.kind, ProviderErrorKind::Quota);
        assert_eq!(
            quota.detail.as_deref(),
            Some("You exceeded your current quota [insufficient_quota]")
        );

        let rate = http_error(StatusCode::TOO_MANY_REQUESTS, "{}", None);
        assert_eq!(rate.kind, ProviderErrorKind::RateLimit);

        let auth = http_error(StatusCode::UNAUTHORIZED, "not json", None);
        assert_eq!(auth.kind, ProviderErrorKind::Auth);
        assert!(auth.message.contains("Settings"));
        // A body that is not JSON is kept as the detail.
        assert_eq!(auth.detail.as_deref(), Some("not json"));

        let missing = http_error(StatusCode::NOT_FOUND, "{}", Some("gpt-9"));
        assert_eq!(
            missing.message,
            "The model gpt-9 is not available at OpenAI."
        );

        assert_eq!(
            http_error(StatusCode::BAD_GATEWAY, "", None).kind,
            ProviderErrorKind::Server
        );
    }

    #[test]
    fn stream_events_are_interpreted() {
        assert_eq!(
            parse_event(r#"{"type":"response.output_text.delta","delta":"Hel","item_id":"x"}"#),
            Some(Ok(StreamEvent::Delta("Hel".into())))
        );
        assert_eq!(
            parse_event(r#"{"type":"response.created","response":{}}"#),
            None
        );
        assert_eq!(parse_event("not json"), None);

        let done = parse_event(
            r#"{"type":"response.completed","response":{"id":"resp_1","model":"gpt-5.6-2026-08-01",
               "usage":{"input_tokens":12,"output_tokens":5,"total_tokens":17}}}"#,
        );
        assert_eq!(
            done,
            Some(Ok(StreamEvent::Done(Completion {
                response_id: Some("resp_1".into()),
                model: Some("gpt-5.6-2026-08-01".into()),
                usage: Some(Usage {
                    input_tokens: 12,
                    output_tokens: 5
                }),
                incomplete_reason: None,
            })))
        );

        let cut = parse_event(
            r#"{"type":"response.incomplete","response":{"id":"r","incomplete_details":{"reason":"max_output_tokens"}}}"#,
        );
        let Some(Ok(StreamEvent::Done(c))) = cut else {
            panic!("{cut:?}")
        };
        assert_eq!(c.incomplete_reason.as_deref(), Some("max_output_tokens"));

        let failed = parse_event(
            r#"{"type":"response.failed","response":{"error":{"code":"server_error","message":"boom"}}}"#,
        );
        let Some(Err(e)) = failed else { panic!() };
        assert_eq!(e.detail.as_deref(), Some("boom [server_error]"));

        let limited =
            parse_event(r#"{"type":"error","code":"rate_limit_exceeded","message":"slow down"}"#);
        let Some(Err(e)) = limited.clone() else {
            panic!()
        };
        assert_eq!(e.detail.as_deref(), Some("slow down [rate_limit_exceeded]"));

        // The same error nested inside an `error` object.
        let nested = parse_event(
            r#"{"type":"error","sequence_number":2,"error":{"type":"invalid_request_error","code":"model_not_found","message":"The model `gpt-x` does not exist","param":"model"}}"#,
        );
        let Some(Err(e)) = nested else { panic!() };
        assert_eq!(e.kind, ProviderErrorKind::ModelUnavailable);
        assert_eq!(
            e.detail.as_deref(),
            Some("The model `gpt-x` does not exist [model_not_found]")
        );

        // An error shaped in a way not seen before keeps the raw event.
        let unknown =
            parse_event(r#"{"type":"error","sequence_number":2,"reason":"something new"}"#);
        let Some(Err(e)) = unknown else { panic!() };
        assert!(e.detail.unwrap().contains("something new"));
        let Some(Err(e)) = limited else { panic!() };
        assert_eq!(e.kind, ProviderErrorKind::RateLimit);
    }
}
