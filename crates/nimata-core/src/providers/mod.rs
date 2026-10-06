//! Model providers: external services that write replies as participants.
//!
//! Each provider adapter translates Nimata's request into its own API and
//! its streamed answer back into [`StreamEvent`]s. Adapters do not try to
//! hide real differences between providers.

pub mod anthropic;
pub mod openai;
pub mod openai_compatible;
pub mod sse;

use std::fmt;
use std::pin::Pin;
use std::sync::Arc;

use futures_util::future::BoxFuture;
use futures_util::{Stream, StreamExt, stream};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::ProviderKind;
use sse::{SseEvent, SseParser};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Anything not written by the responding model.
    User,
    /// Earlier posts by the responding model.
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub text: String,
}

/// What is sent to a provider for one reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRequest {
    pub model: String,
    pub instructions: String,
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// How a reply finished, as reported by the provider.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Completion {
    /// The provider's own ID for this response.
    pub response_id: Option<String>,
    /// The exact model version that answered.
    pub model: Option<String>,
    pub usage: Option<Usage>,
    /// Set when the reply stopped early, e.g. at the output token limit.
    pub incomplete_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    /// More text of the reply.
    Delta(String),
    /// The reply is finished.
    Done(Completion),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderErrorKind {
    /// The API key is missing, wrong, or not allowed to do this.
    Auth,
    /// Too many requests right now.
    RateLimit,
    /// The account has no credit left.
    Quota,
    /// The model does not exist or this key cannot use it.
    ModelUnavailable,
    /// The provider rejected the request.
    BadRequest,
    /// The provider failed.
    Server,
    /// The provider could not be reached.
    Network,
    /// The connection ended before the reply finished.
    Interrupted,
    /// The model declined to answer.
    Refused,
}

/// A provider failure, with a message written for the user. `detail` keeps
/// the provider's own wording, and `request_id` its request ID for support.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderError {
    pub kind: ProviderErrorKind,
    pub message: String,
    pub detail: Option<String>,
    pub request_id: Option<String>,
}

impl ProviderError {
    pub fn new(kind: ProviderErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            detail: None,
            request_id: None,
        }
    }

    pub fn with_detail(mut self, detail: Option<String>) -> Self {
        self.detail = detail.filter(|d| !d.trim().is_empty());
        self
    }

    pub fn with_request_id(mut self, request_id: Option<String>) -> Self {
        self.request_id = request_id;
        self
    }
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)?;
        if let Some(detail) = &self.detail {
            write!(f, " ({detail})")?;
        }
        Ok(())
    }
}

impl std::error::Error for ProviderError {}

pub type EventStream = Pin<Box<dyn Stream<Item = Result<StreamEvent, ProviderError>> + Send>>;

/// A reply being streamed back.
pub struct ResponseStream {
    /// The provider's request ID from the response headers, if any.
    pub request_id: Option<String>,
    pub events: EventStream,
}

pub trait ModelProvider: Send + Sync {
    /// Models this provider offers that can write text replies.
    fn models(&self) -> BoxFuture<'_, Result<Vec<ModelInfo>, ProviderError>>;

    /// Starts a reply; text arrives through the returned stream.
    fn respond(
        &self,
        request: ModelRequest,
    ) -> BoxFuture<'_, Result<ResponseStream, ProviderError>>;
}

pub use reqwest::Client as HttpClient;

/// What a kind of provider can do, so the UI offers only what works.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    /// An API key is needed (local OpenAI-compatible servers often need none).
    pub requires_key: bool,
    /// The endpoint can be changed; true for OpenAI-compatible servers.
    pub custom_endpoint: bool,
    /// Several connections of this kind can exist at once.
    pub multiple: bool,
    /// The provider lists its models.
    pub model_discovery: bool,
    /// Replies stream in as they are written.
    pub streaming: bool,
    /// Token counts are reported.
    pub usage: bool,
}

pub fn capabilities(kind: ProviderKind) -> Capabilities {
    match kind {
        ProviderKind::OpenAi | ProviderKind::Anthropic => Capabilities {
            requires_key: true,
            custom_endpoint: false,
            multiple: false,
            model_discovery: true,
            streaming: true,
            usage: true,
        },
        ProviderKind::OpenAiCompatible => Capabilities {
            requires_key: false,
            custom_endpoint: true,
            multiple: true,
            model_discovery: true,
            streaming: true,
            // Many servers report usage only when asked, and some never do.
            usage: false,
        },
    }
}

/// How much context, in estimated tokens, Nimata sends to a model before
/// trimming the middle of a long thread. Hosted models accept far more;
/// local models often have small context windows.
pub fn context_budget(kind: ProviderKind) -> u64 {
    match kind {
        ProviderKind::OpenAi | ProviderKind::Anthropic => 100_000,
        ProviderKind::OpenAiCompatible => 6_000,
    }
}

/// The error code and message from a provider's error body. Understands
/// `{"error": {"message", "code"|"type"}}` (OpenAI, Anthropic, most
/// OpenAI-compatible servers), `{"error": "text"}`, and `{"message": "text"}`.
pub fn error_body(body: &str) -> (String, Option<String>) {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        let text = body.trim();
        return (
            String::new(),
            (!text.is_empty()).then(|| text.chars().take(300).collect()),
        );
    };
    let text = |v: Option<&Value>| v.and_then(Value::as_str).map(str::to_string);
    match value.get("error") {
        Some(Value::String(message)) => (String::new(), Some(message.clone())),
        Some(error) => (
            text(error.get("code"))
                .or_else(|| text(error.get("type")))
                .unwrap_or_default(),
            text(error.get("message")),
        ),
        None => (String::new(), text(value.get("message"))),
    }
}

/// Turns an HTTP error status into a message the user can act on.
pub fn status_error(provider: &str, status: u16, body: &str, model: Option<&str>) -> ProviderError {
    let (code, detail) = error_body(body);
    let model = model.unwrap_or("this model");
    let (kind, message) = match status {
        401 => (
            ProviderErrorKind::Auth,
            format!("{provider} rejected the API key. Check it in Settings, Models."),
        ),
        402 => (
            ProviderErrorKind::Quota,
            format!("The {provider} account has a billing problem or no remaining credit."),
        ),
        403 => (
            ProviderErrorKind::Auth,
            format!("This API key is not allowed to use {model}."),
        ),
        404 => (
            ProviderErrorKind::ModelUnavailable,
            format!("The model {model} is not available at {provider}."),
        ),
        429 if code == "insufficient_quota" => (
            ProviderErrorKind::Quota,
            format!("The {provider} account has run out of credit or reached its spending limit."),
        ),
        429 => (
            ProviderErrorKind::RateLimit,
            format!("{provider} is limiting requests right now. Wait a moment, then retry."),
        ),
        529 => (
            ProviderErrorKind::Server,
            format!("{provider} is overloaded right now. Retry in a moment."),
        ),
        400..=499 => (
            ProviderErrorKind::BadRequest,
            format!("{provider} could not accept this request."),
        ),
        _ => (
            ProviderErrorKind::Server,
            format!("{provider} had a server problem. Retry in a moment."),
        ),
    };
    let detail = match (detail, code.as_str()) {
        (Some(detail), "") => Some(detail),
        (Some(detail), code) => Some(format!("{detail} [{code}]")),
        (None, _) => None,
    };
    ProviderError::new(kind, message).with_detail(detail)
}

pub fn network_error(provider: &str, error: &reqwest::Error) -> ProviderError {
    ProviderError::new(
        ProviderErrorKind::Network,
        format!("Could not reach {provider}. Check the connection, then retry."),
    )
    .with_detail(Some(error.to_string()))
}

pub fn interrupted(provider: &str) -> ProviderError {
    ProviderError::new(
        ProviderErrorKind::Interrupted,
        format!("The connection to {provider} ended before the reply was finished."),
    )
}

pub fn header(response: &reqwest::Response, name: &str) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
}

/// Turns a streamed HTTP body into Nimata events. `parse` interprets each
/// server-sent event (and may keep state across them); `None` skips one.
/// The stream ends after a `Done` or an error; ending without either means
/// the reply was cut off.
pub fn sse_stream<P>(response: reqwest::Response, provider: String, parse: P) -> EventStream
where
    P: FnMut(&SseEvent) -> Option<Result<StreamEvent, ProviderError>> + Send + 'static,
{
    struct State<B, P> {
        bytes: B,
        parser: SseParser,
        parse: P,
        pending: std::collections::VecDeque<Result<StreamEvent, ProviderError>>,
        finished: bool,
        provider: String,
    }
    let state = State {
        bytes: response.bytes_stream(),
        parser: SseParser::new(),
        parse,
        pending: Default::default(),
        finished: false,
        provider,
    };
    Box::pin(stream::unfold(state, |mut s| async move {
        loop {
            if let Some(event) = s.pending.pop_front() {
                if matches!(event, Ok(StreamEvent::Done(_)) | Err(_)) {
                    s.finished = true;
                    s.pending.clear();
                }
                return Some((event, s));
            }
            if s.finished {
                return None;
            }
            match s.bytes.next().await {
                Some(Ok(chunk)) => {
                    for event in s.parser.push(&chunk) {
                        if let Some(parsed) = (s.parse)(&event) {
                            s.pending.push_back(parsed);
                        }
                    }
                }
                Some(Err(e)) => {
                    s.finished = true;
                    return Some((Err(network_error(&s.provider, &e)), s));
                }
                None => {
                    s.finished = true;
                    return Some((Err(interrupted(&s.provider)), s));
                }
            }
        }
    }))
}

/// Makes `ring` the process-wide TLS crypto provider. reqwest is built with
/// `rustls-no-provider` (so nothing needs CMake on mobile), and Cargo applies
/// that feature to every reqwest client in the app, including ones Tauri
/// creates itself, such as the mobile dev-server proxy. Without a default
/// provider those panic on first use, so install one at startup. Safe to
/// call more than once.
pub fn install_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

/// The HTTP client used for every provider. TLS uses rustls with the
/// `ring` backend and Mozilla's root certificates, which builds and behaves
/// the same on desktop and mobile.
pub fn http_client() -> reqwest::Client {
    install_crypto_provider();
    let roots = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    let tls = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .expect("ring supports the default protocol versions")
    .with_root_certificates(roots)
    .with_no_client_auth();
    reqwest::Client::builder()
        .tls_backend_preconfigured(tls)
        .connect_timeout(std::time::Duration::from_secs(20))
        .user_agent(concat!("Nimata/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("the HTTP client configuration is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_http_client_works_once_the_provider_is_installed() {
        install_crypto_provider();
        install_crypto_provider();
        // A client built without Nimata's own TLS setup, as Tauri does.
        let _ = reqwest::Client::new();
        let _ = http_client();
    }
}
