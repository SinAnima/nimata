//! Model providers: external services that write replies as participants.
//!
//! Each provider adapter translates Nimata's request into its own API and
//! its streamed answer back into [`StreamEvent`]s. Adapters do not try to
//! hide real differences between providers.

pub mod openai;
pub mod sse;

use std::fmt;
use std::pin::Pin;
use std::sync::Arc;

use futures_util::Stream;
use futures_util::future::BoxFuture;
use serde::{Deserialize, Serialize};

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

/// The HTTP client used for every provider. TLS uses rustls with the
/// `ring` backend and Mozilla's root certificates, which builds and behaves
/// the same on desktop and mobile.
pub fn http_client() -> reqwest::Client {
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
