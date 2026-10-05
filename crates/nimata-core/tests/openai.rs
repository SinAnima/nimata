//! The OpenAI adapter against a local HTTP server that replays recorded
//! responses. No network access and no API key are needed.

use futures_util::StreamExt;
use nimata_core::providers::openai::OpenAi;
use nimata_core::providers::{
    Message, ModelProvider, ModelRequest, ProviderErrorKind, Role, StreamEvent, Usage, http_client,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

/// What the server received.
#[derive(Debug)]
struct Received {
    head: String,
    body: String,
}

/// Serves one HTTP request with `status`, `headers`, and `chunks` written
/// with a short pause between them, then closes the connection.
async fn serve_once(
    status: &'static str,
    headers: &'static str,
    chunks: Vec<String>,
) -> (String, oneshot::Receiver<Received>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let (tx, rx) = oneshot::channel();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = socket.read(&mut buf).await.unwrap();
            request.extend_from_slice(&buf[..n]);
            let text = String::from_utf8_lossy(&request).to_string();
            if let Some(end) = text.find("\r\n\r\n") {
                let length = text[..end]
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                if request.len() >= end + 4 + length {
                    let _ = tx.send(Received {
                        head: text[..end].to_string(),
                        body: text[end + 4..].to_string(),
                    });
                    break;
                }
            }
            if n == 0 {
                break;
            }
        }
        let head = format!("HTTP/1.1 {status}\r\n{headers}Connection: close\r\n\r\n");
        socket.write_all(head.as_bytes()).await.unwrap();
        for chunk in chunks {
            socket.write_all(chunk.as_bytes()).await.unwrap();
            socket.flush().await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        }
    });
    (base, rx)
}

fn request() -> ModelRequest {
    ModelRequest {
        model: "gpt-5.6".into(),
        instructions: "You are GPT-5.6.".into(),
        messages: vec![Message {
            role: Role::User,
            text: "Thanos:\nShould Nimata use CouchDB?".into(),
        }],
    }
}

/// Splits text into small uneven chunks, the way a network delivers it.
fn chunked(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    bytes
        .chunks(37)
        .map(|c| String::from_utf8_lossy(c).into_owned())
        .collect()
}

const RECORDED: &str = include_str!("fixtures/openai_responses_stream.sse");

#[tokio::test]
async fn streams_a_recorded_reply_and_its_usage() {
    let (base, received) = serve_once(
        "200 OK",
        "Content-Type: text/event-stream\r\nx-request-id: req_42\r\n",
        chunked(RECORDED),
    )
    .await;
    let provider = OpenAi::new(http_client(), &base, "sk-test".into());

    let response = provider.respond(request()).await.unwrap();
    assert_eq!(response.request_id.as_deref(), Some("req_42"));
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
        "Replication is the attraction, but mobile changes the constraints."
    );
    let Some(Ok(StreamEvent::Done(done))) = events.last() else {
        panic!("{:?}", events.last())
    };
    assert_eq!(done.response_id.as_deref(), Some("resp_0abc"));
    assert_eq!(done.model.as_deref(), Some("gpt-5.6-2026-08-01"));
    assert_eq!(
        done.usage,
        Some(Usage {
            input_tokens: 41,
            output_tokens: 12
        })
    );

    let received = received.await.unwrap();
    assert!(
        received.head.starts_with("POST /v1/responses"),
        "{}",
        received.head
    );
    assert!(
        received
            .head
            .to_ascii_lowercase()
            .contains("authorization: bearer sk-test")
    );
    let body: serde_json::Value = serde_json::from_str(&received.body).unwrap();
    assert_eq!(body["store"], false);
    assert_eq!(body["model"], "gpt-5.6");
}

#[tokio::test]
async fn a_stream_that_stops_early_is_reported_as_interrupted() {
    let cut = &RECORDED[..RECORDED.find("response.output_text.done").unwrap()];
    let (base, _) = serve_once(
        "200 OK",
        "Content-Type: text/event-stream\r\n",
        chunked(cut),
    )
    .await;
    let provider = OpenAi::new(http_client(), &base, "sk-test".into());

    let events: Vec<_> = provider
        .respond(request())
        .await
        .unwrap()
        .events
        .collect()
        .await;
    let deltas = events
        .iter()
        .filter(|e| matches!(e, Ok(StreamEvent::Delta(_))))
        .count();
    assert_eq!(deltas, 6, "text received before the cut is still delivered");
    let Some(Err(error)) = events.last() else {
        panic!("{:?}", events.last())
    };
    assert_eq!(error.kind, ProviderErrorKind::Interrupted);
}

#[tokio::test]
async fn a_rejected_key_is_reported_with_the_request_id() {
    let (base, _) = serve_once(
        "401 Unauthorized",
        "Content-Type: application/json\r\nx-request-id: req_bad\r\n",
        vec![r#"{"error":{"message":"Incorrect API key provided: sk-tes*","type":"invalid_request_error","code":"invalid_api_key"}}"#.into()],
    )
    .await;
    let provider = OpenAi::new(http_client(), &base, "sk-test".into());
    let error = match provider.respond(request()).await {
        Err(e) => e,
        Ok(_) => panic!("expected an error"),
    };
    assert_eq!(error.kind, ProviderErrorKind::Auth);
    assert_eq!(error.request_id.as_deref(), Some("req_bad"));
    assert!(error.detail.unwrap().contains("Incorrect API key"));
}

#[tokio::test]
async fn an_unreachable_server_is_a_network_error() {
    // Bind and drop a listener so the port is closed.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    drop(listener);
    let provider = OpenAi::new(http_client(), &base, "sk-test".into());
    let Err(error) = provider.models().await else {
        panic!("expected an error")
    };
    assert_eq!(error.kind, ProviderErrorKind::Network);
}

#[tokio::test]
async fn lists_only_text_models_in_order() {
    let (base, received) = serve_once(
        "200 OK",
        "Content-Type: application/json\r\n",
        vec![
            r#"{"object":"list","data":[
            {"id":"whisper-1","object":"model"},
            {"id":"gpt-5.6-mini","object":"model"},
            {"id":"text-embedding-3-small","object":"model"},
            {"id":"gpt-5.6","object":"model"}]}"#
                .into(),
        ],
    )
    .await;
    let provider = OpenAi::new(http_client(), &base, "sk-test".into());
    let ids: Vec<_> = provider
        .models()
        .await
        .unwrap()
        .into_iter()
        .map(|m| m.id)
        .collect();
    assert_eq!(ids, vec!["gpt-5.6", "gpt-5.6-mini"]);
    assert!(received.await.unwrap().head.starts_with("GET /v1/models"));
}
