//! The Anthropic adapter against a local server replaying a recorded stream.

use futures_util::StreamExt;
use nimata_core::providers::anthropic::Anthropic;
use nimata_core::providers::{
    Message, ModelProvider, ModelRequest, ProviderErrorKind, Role, StreamEvent, Usage, http_client,
};

mod support;
use support::{chunked, serve_once};

const RECORDED: &str = include_str!("fixtures/anthropic_messages_stream.sse");

fn request() -> ModelRequest {
    ModelRequest {
        model: "claude-opus-5-5".into(),
        instructions: "You are Claude Opus.".into(),
        messages: vec![Message {
            role: Role::User,
            text: "Thanos:\nCan sync stay an adapter?".into(),
            files: vec![],
        }],
    }
}

#[tokio::test]
async fn streams_a_recorded_reply_skipping_thinking() {
    let (base, received) = serve_once(
        "200 OK",
        "Content-Type: text/event-stream\r\nrequest-id: req_anthropic_1\r\n",
        chunked(RECORDED),
    )
    .await;
    let provider = Anthropic::new(http_client(), &base, "sk-ant-test".into());
    let response = provider.respond(request()).await.unwrap();
    assert_eq!(response.request_id.as_deref(), Some("req_anthropic_1"));
    let events: Vec<_> = response.events.collect().await;

    let text: String = events
        .iter()
        .filter_map(|e| match e {
            Ok(StreamEvent::Delta(d)) => Some(d.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        text,
        "Sync can stay an adapter if the store stays authoritative."
    );
    let Some(Ok(StreamEvent::Done(done))) = events.last() else {
        panic!("{:?}", events.last())
    };
    assert_eq!(done.model.as_deref(), Some("claude-opus-5-5"));
    assert_eq!(done.response_id.as_deref(), Some("msg_01abc"));
    assert_eq!(
        done.usage,
        Some(Usage {
            input_tokens: 52,
            output_tokens: 18
        })
    );

    let received = received.await.unwrap();
    let head = received.head.to_ascii_lowercase();
    assert!(
        received.head.starts_with("POST /v1/messages"),
        "{}",
        received.head
    );
    assert!(head.contains("x-api-key: sk-ant-test"));
    assert!(head.contains("anthropic-version: 2023-06-01"));
    let body: serde_json::Value = serde_json::from_str(&received.body).unwrap();
    assert_eq!(body["system"], "You are Claude Opus.");
    assert_eq!(body["stream"], true);
}

#[tokio::test]
async fn an_overloaded_api_is_reported_with_its_request_id() {
    let (base, _) = serve_once(
        "529 Overloaded",
        "Content-Type: application/json\r\nrequest-id: req_busy\r\n",
        vec![r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"},"request_id":"req_busy"}"#.into()],
    )
    .await;
    let provider = Anthropic::new(http_client(), &base, "sk-ant-test".into());
    let Err(error) = provider.respond(request()).await else {
        panic!("expected an error")
    };
    assert_eq!(error.kind, ProviderErrorKind::Server);
    assert_eq!(
        error.message,
        "Anthropic is overloaded right now. Retry in a moment."
    );
    assert_eq!(error.request_id.as_deref(), Some("req_busy"));
    assert_eq!(
        error.detail.as_deref(),
        Some("Overloaded [overloaded_error]")
    );
}

#[tokio::test]
async fn lists_models() {
    let (base, received) = serve_once(
        "200 OK",
        "Content-Type: application/json\r\n",
        vec![r#"{"data":[{"type":"model","id":"claude-opus-5-5","display_name":"Claude Opus 5.5"},
                         {"type":"model","id":"claude-sonnet-5-5","display_name":"Claude Sonnet 5.5"}],
                 "has_more":false,"first_id":"claude-opus-5-5","last_id":"claude-sonnet-5-5"}"#.into()],
    )
    .await;
    let provider = Anthropic::new(http_client(), &base, "sk-ant-test".into());
    let ids: Vec<_> = provider
        .models()
        .await
        .unwrap()
        .into_iter()
        .map(|m| m.id)
        .collect();
    assert_eq!(ids, vec!["claude-opus-5-5", "claude-sonnet-5-5"]);
    assert!(
        received
            .await
            .unwrap()
            .head
            .starts_with("GET /v1/models?limit=1000")
    );
}
