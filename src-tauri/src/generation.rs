//! Running model replies: build the request, stream the reply into its post,
//! and record how it ended. Progress reaches the UI as events, so any open
//! view, or one opened later, sees the same post.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use nimata_core::context::build_request;
use nimata_core::domain::{GenerationStatus, Post, ProviderConfig, ProviderKind, ProviderMetadata};
use nimata_core::providers::openai::{DEFAULT_BASE_URL, OpenAi};
use nimata_core::providers::{
    Completion, HttpClient, ModelProvider, ModelRequest, ProviderError, StreamEvent, http_client,
};
use nimata_core::repository::GenerationOutcome;
use nimata_core::{Repository, Timestamp, UnixMillis};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::secrets::Keys;
use crate::state::AppState;

/// Emitted with each piece of streamed text.
pub const POST_DELTA: &str = "nimata://post-delta";
/// Emitted with the whole post when a reply starts or ends.
pub const POST_UPDATED: &str = "nimata://post-updated";

/// How often text received so far is saved, so a crash loses little.
const SAVE_EVERY: Duration = Duration::from_millis(300);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostDelta {
    pub post_id: Uuid,
    pub text: String,
}

pub struct Generations {
    client: HttpClient,
    /// Replies in progress, by post ID, with the signal that stops them.
    running: Mutex<HashMap<Uuid, oneshot::Sender<()>>>,
}

impl Default for Generations {
    fn default() -> Self {
        Self {
            client: http_client(),
            running: Mutex::new(HashMap::new()),
        }
    }
}

impl Generations {
    /// Stops a reply in progress. Its text so far is kept.
    pub fn cancel(&self, post_id: Uuid) -> bool {
        match self.running.lock().unwrap().remove(&post_id) {
            Some(stop) => stop.send(()).is_ok(),
            None => false,
        }
    }

    pub fn provider_for(
        &self,
        config: &ProviderConfig,
        key: String,
    ) -> Result<Box<dyn ModelProvider>, String> {
        match config.kind {
            ProviderKind::OpenAi => Ok(Box::new(OpenAi::new(
                self.client.clone(),
                config.base_url.as_deref().unwrap_or(DEFAULT_BASE_URL),
                key,
            ))),
            other => Err(format!(
                "{} replies arrive in a later version of Nimata",
                other.as_str()
            )),
        }
    }
}

fn text<E: ToString>(e: E) -> String {
    e.to_string()
}

/// Asks the model participant `participant_id` to reply to `parent_id`.
/// Returns the new reply, which then streams in the background.
pub fn ask<R: Runtime>(
    app: &AppHandle<R>,
    discussion_id: Uuid,
    parent_id: Uuid,
    participant_id: Uuid,
) -> Result<Post, String> {
    let state = app.state::<AppState>();
    let keys = app.state::<Keys>();
    let generations = app.state::<Generations>();

    let (provider, provider_name, request, post, generation_id) = {
        let mut repo = state.repo()?;
        let model = repo
            .model_participants()
            .map_err(text)?
            .into_iter()
            .find(|m| m.participant.id == participant_id)
            .ok_or("this model is no longer set up in Settings")?;
        if !model.enabled {
            return Err(format!(
                "{} is turned off in Settings, Models",
                model.participant.display_name
            ));
        }
        let config = repo.provider(model.provider_id).map_err(text)?;
        let Some((key, _)) = keys.resolve(&config)? else {
            return Err(format!(
                "Add an {} API key in Settings, Models.",
                config.display_name
            ));
        };
        let provider = generations.provider_for(&config, key)?;
        let provider_name = config.kind.as_str().to_string();

        let view = repo.get_discussion(discussion_id).map_err(text)?;
        let mut participants = view.participants.clone();
        participants.push(model.participant.clone());
        let model_id = model.participant.model.clone().unwrap_or_default();
        let (request, context) = build_request(
            &view.posts,
            &participants,
            parent_id,
            &model.participant,
            &model_id,
        )
        .map_err(text)?;
        let (post, generation) = repo
            .begin_generation(
                discussion_id,
                parent_id,
                participant_id,
                &context,
                Timestamp::now(),
            )
            .map_err(text)?;
        (provider, provider_name, request, post, generation.id)
    };

    let (stop, stopped) = oneshot::channel();
    generations.running.lock().unwrap().insert(post.id, stop);
    let _ = app.emit(POST_UPDATED, &post);

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let outcome = stream_reply(
            &app,
            provider,
            request,
            post.id,
            generation_id,
            provider_name,
            stopped,
        )
        .await;
        app.state::<Generations>()
            .running
            .lock()
            .unwrap()
            .remove(&post.id);
        let finished = app.state::<AppState>().repo().and_then(|mut repo| {
            repo.finish_generation(generation_id, outcome, UnixMillis::now())
                .map_err(text)
        });
        if let Ok(post) = finished {
            let _ = app.emit(POST_UPDATED, &post);
        }
    });
    Ok(post)
}

fn metadata(provider: String, request_id: Option<String>, done: Completion) -> ProviderMetadata {
    ProviderMetadata {
        provider,
        model: done.model,
        response_id: done.response_id,
        request_id,
        input_tokens: done.usage.map(|u| u.input_tokens),
        output_tokens: done.usage.map(|u| u.output_tokens),
        incomplete_reason: done.incomplete_reason,
    }
}

/// A failed reply keeps the provider's request ID, when there is one, so
/// the failure can be traced with the provider.
fn failed(
    body: String,
    error: &ProviderError,
    provider: &str,
    request_id: Option<String>,
) -> GenerationOutcome {
    let request_id = error.request_id.clone().or(request_id);
    GenerationOutcome {
        status: GenerationStatus::Failed,
        body,
        metadata: request_id.map(|request_id| ProviderMetadata {
            provider: provider.to_string(),
            model: None,
            response_id: None,
            request_id: Some(request_id),
            input_tokens: None,
            output_tokens: None,
            incomplete_reason: None,
        }),
        error: Some(error.to_string()),
    }
}

fn cancelled(body: String) -> GenerationOutcome {
    GenerationOutcome {
        status: GenerationStatus::Cancelled,
        body,
        metadata: None,
        error: None,
    }
}

async fn stream_reply<R: Runtime>(
    app: &AppHandle<R>,
    provider: Box<dyn ModelProvider>,
    request: ModelRequest,
    post_id: Uuid,
    generation_id: Uuid,
    provider_name: String,
    mut stopped: oneshot::Receiver<()>,
) -> GenerationOutcome {
    let save = |status: GenerationStatus, body: &str| {
        if let Ok(mut repo) = app.state::<AppState>().repo() {
            let _ = repo.update_generation(generation_id, status, body);
        }
    };

    let response = tokio::select! {
        response = provider.respond(request) => response,
        _ = &mut stopped => return cancelled(String::new()),
    };
    let mut response = match response {
        Ok(response) => response,
        Err(error) => return failed(String::new(), &error, &provider_name, None),
    };
    save(GenerationStatus::Streaming, "");

    let mut body = String::new();
    // Text received but not yet saved. It is saved within SAVE_EVERY even if
    // the stream then stalls, so a crash loses at most that much.
    let mut unsaved = false;
    let mut last_save = Instant::now();
    loop {
        let save_due = last_save + SAVE_EVERY;
        let event = tokio::select! {
            event = response.events.next() => event,
            _ = tokio::time::sleep_until(save_due.into()), if unsaved => {
                save(GenerationStatus::Streaming, &body);
                unsaved = false;
                last_save = Instant::now();
                continue;
            }
            _ = &mut stopped => return cancelled(body),
        };
        match event {
            Some(Ok(StreamEvent::Delta(delta))) => {
                body.push_str(&delta);
                unsaved = true;
                let _ = app.emit(
                    POST_DELTA,
                    PostDelta {
                        post_id,
                        text: delta,
                    },
                );
                if last_save.elapsed() >= SAVE_EVERY {
                    save(GenerationStatus::Streaming, &body);
                    unsaved = false;
                    last_save = Instant::now();
                }
            }
            Some(Ok(StreamEvent::Done(done))) => {
                return GenerationOutcome {
                    status: GenerationStatus::Complete,
                    body,
                    metadata: Some(metadata(provider_name, response.request_id, done)),
                    error: None,
                };
            }
            Some(Err(error)) => {
                return failed(body, &error, &provider_name, response.request_id);
            }
            None => {
                return failed(
                    body,
                    &ProviderError::new(
                        nimata_core::providers::ProviderErrorKind::Interrupted,
                        "The reply ended unexpectedly.",
                    ),
                    &provider_name,
                    response.request_id,
                );
            }
        }
    }
}
