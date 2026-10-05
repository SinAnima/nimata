//! Talks to the real OpenAI API. Ignored by default: it needs a key, the
//! network, and costs a fraction of a cent. Run it explicitly with:
//!
//!     OPENAI_API_KEY=sk-... cargo test -p nimata-core --test openai_live -- --ignored
//!
//! NIMATA_LIVE_MODEL picks the model; otherwise the first "mini" model listed.

use futures_util::StreamExt;
use nimata_core::providers::openai::{DEFAULT_BASE_URL, OpenAi};
use nimata_core::providers::{
    Message, ModelProvider, ModelRequest, Role, StreamEvent, http_client,
};

#[tokio::test]
#[ignore = "talks to the real OpenAI API; needs OPENAI_API_KEY"]
async fn a_real_reply_streams_over_tls() {
    let key = std::env::var("OPENAI_API_KEY").expect("set OPENAI_API_KEY to run this test");
    let provider = OpenAi::new(http_client(), DEFAULT_BASE_URL, key);

    let models = provider.models().await.expect("the key lists models");
    assert!(!models.is_empty());
    let model = std::env::var("NIMATA_LIVE_MODEL").unwrap_or_else(|_| {
        models
            .iter()
            .find(|m| m.id.contains("mini"))
            .unwrap_or(&models[0])
            .id
            .clone()
    });

    let response = provider
        .respond(ModelRequest {
            model: model.clone(),
            instructions: "Reply with exactly one word.".into(),
            messages: vec![Message {
                role: Role::User,
                text: "Tester:\nSay hello.".into(),
            }],
        })
        .await
        .expect("the request is accepted");
    let events: Vec<_> = response.events.collect().await;
    let text: String = events
        .iter()
        .filter_map(|e| match e {
            Ok(StreamEvent::Delta(d)) => Some(d.as_str()),
            _ => None,
        })
        .collect();
    assert!(!text.trim().is_empty(), "{events:?}");
    assert!(
        matches!(events.last(), Some(Ok(StreamEvent::Done(_)))),
        "{model}: {:?}",
        events.last()
    );
}
