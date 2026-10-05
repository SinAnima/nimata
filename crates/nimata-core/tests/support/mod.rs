//! A one-request local HTTP server for provider tests: it records what it
//! receives and replies with canned status, headers, and body chunks.

#![allow(dead_code)]

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

/// What the server received.
#[derive(Debug)]
pub struct Received {
    pub head: String,
    pub body: String,
}

/// Serves one HTTP request with `status`, `headers`, and `chunks` written
/// with a short pause between them, then closes the connection.
pub async fn serve_once(
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

/// Splits text into small uneven chunks, the way a network delivers it.
pub fn chunked(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    bytes
        .chunks(37)
        .map(|c| String::from_utf8_lossy(c).into_owned())
        .collect()
}
