//! The OpenAI-compatible adapter against a local server replaying a
//! recorded Chat Completions stream.

use futures_util::StreamExt;
use nimata_core::providers::openai_compatible::OpenAiCompatible;
use nimata_core::providers::{
    Message, ModelProvider, ModelRequest, ProviderErrorKind, Role, StreamEvent, Usage, http_client,
};

mod support;
use support::{chunked, serve_once};

const RECORDED: &str = include_str!("fixtures/openai_compatible_stream.sse");

fn request() -> ModelRequest {
    ModelRequest {
        model: "qwen3:8b".into(),
        instructions: "You are Qwen.".into(),
        messages: vec![Message {
            role: Role::User,
            text: "Thanos:\nCan local models join?".into(),
            files: vec![],
        }],
    }
}

#[tokio::test]
async fn streams_a_reply_without_a_key() {
    let (base, received) = serve_once(
        "200 OK",
        "Content-Type: text/event-stream\r\n",
        chunked(RECORDED),
    )
    .await;
    let provider = OpenAiCompatible::new(http_client(), &base, None, "Ollama");
    let events: Vec<_> = provider
        .respond(request())
        .await
        .unwrap()
        .events
        .collect()
        .await;

    let text: String = events
        .iter()
        .filter_map(|e| match e {
            Ok(StreamEvent::Delta(d)) => Some(d.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(text, "Local models can join too.");
    let Some(Ok(StreamEvent::Done(done))) = events.last() else {
        panic!("{:?}", events.last())
    };
    assert_eq!(
        done.usage,
        Some(Usage {
            input_tokens: 44,
            output_tokens: 6
        })
    );
    assert_eq!(done.incomplete_reason, None);

    let received = received.await.unwrap();
    assert!(
        received.head.starts_with("POST /v1/chat/completions"),
        "{}",
        received.head
    );
    assert!(
        !received.head.to_ascii_lowercase().contains("authorization"),
        "no key, no header"
    );
}

#[tokio::test]
async fn a_server_that_rejects_the_usage_option_is_asked_again_without_it() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let mut bodies = Vec::new();
        for reply in [
            "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"error\":{\"message\":\"unknown field: stream_options\"}}".to_string(),
            format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n{RECORDED}"),
        ] {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 65536];
            let mut request = Vec::new();
            loop {
                let n = socket.read(&mut buf).await.unwrap();
                request.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&request).to_string();
                if let Some(end) = text.find("\r\n\r\n") {
                    let length: usize = text[..end]
                        .lines()
                        .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap()))
                        .unwrap_or(0);
                    if request.len() >= end + 4 + length {
                        bodies.push(text[end + 4..].to_string());
                        break;
                    }
                }
            }
            socket.write_all(reply.as_bytes()).await.unwrap();
        }
        bodies
    });
    let provider = OpenAiCompatible::new(http_client(), &base, Some("key".into()), "vLLM");
    let events: Vec<_> = provider
        .respond(request())
        .await
        .unwrap()
        .events
        .collect()
        .await;
    assert!(matches!(events.last(), Some(Ok(StreamEvent::Done(_)))));
    let bodies = server.await.unwrap();
    assert!(bodies[0].contains("stream_options"));
    assert!(!bodies[1].contains("stream_options"));
}

#[tokio::test]
async fn errors_name_the_connection() {
    let (base, _) = serve_once(
        "404 Not Found",
        "Content-Type: application/json\r\n",
        vec![r#"{"error":{"message":"model \"qwen9\" not found, try pulling it first"}}"#.into()],
    )
    .await;
    let provider = OpenAiCompatible::new(http_client(), &base, None, "Ollama");
    let Err(error) = provider.respond(request()).await else {
        panic!("expected an error")
    };
    assert_eq!(error.kind, ProviderErrorKind::ModelUnavailable);
    assert_eq!(
        error.message,
        "The model qwen3:8b is not available at Ollama."
    );
    assert!(error.detail.unwrap().contains("try pulling it first"));
}

#[tokio::test]
async fn lists_every_model_and_sends_the_key_when_there_is_one() {
    let (base, received) = serve_once(
        "200 OK",
        "Content-Type: application/json\r\n",
        vec![r#"{"object":"list","data":[{"id":"qwen3:8b"},{"id":"llama3.3:70b"}]}"#.into()],
    )
    .await;
    let provider = OpenAiCompatible::new(http_client(), &base, Some("local-key".into()), "vLLM");
    let ids: Vec<_> = provider
        .models()
        .await
        .unwrap()
        .into_iter()
        .map(|m| m.id)
        .collect();
    assert_eq!(ids, vec!["llama3.3:70b", "qwen3:8b"], "every model, sorted");
    let head = received.await.unwrap().head.to_ascii_lowercase();
    assert!(head.starts_with("get /v1/models"));
    assert!(head.contains("authorization: bearer local-key"));
}

#[tokio::test]
async fn a_server_that_is_not_openai_compatible_is_explained() {
    let (base, _) = serve_once(
        "200 OK",
        "Content-Type: text/html\r\n",
        vec!["<html>Welcome</html>".into()],
    )
    .await;
    let provider = OpenAiCompatible::new(http_client(), &base, None, "Router");
    let Err(error) = provider.models().await else {
        panic!("expected an error")
    };
    assert!(
        error.message.contains("Check the endpoint address"),
        "{error}"
    );
}
