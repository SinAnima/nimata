//! Reads ChatGPT's data export (`conversations.json`) into discussions.
//!
//! Each conversation is a tree of messages (`mapping`): editing a prompt or
//! regenerating an answer starts a new branch. Every branch is kept, as
//! replies to the same post, so nothing is lost. System, tool, and hidden
//! messages are left out; a kept message replies to its nearest kept
//! ancestor.
//!
//! Nimata IDs are derived from ChatGPT's IDs (name-based UUIDs), so
//! importing the same export again adds only what is new. ChatGPT's own IDs
//! are kept in the answers' provider metadata. The export has no time
//! zones, so times are recorded in UTC.

use std::collections::HashMap;

use serde_json::Value;
use uuid::Uuid;

use crate::archive::{
    ArchivedDiscussion, ArchivedParticipant, ArchivedPost, DiscussionArchive, FORMAT, format_utc,
};
use crate::domain::{ParticipantKind, PostStatus, ProviderMetadata};
use crate::error::{Error, Result};
use crate::time::UnixMillis;

/// Namespace for IDs derived from ChatGPT's. Never change it: it is what
/// makes repeated imports land on the same Nimata IDs.
const NAMESPACE: Uuid = Uuid::from_u128(0x6e1d_7a3c_51f2_4b8e_9d0a_c4e2_7f13_b5a9);

fn derived_id(kind: &str, foreign: &str) -> Uuid {
    Uuid::new_v5(&NAMESPACE, format!("chatgpt:{kind}:{foreign}").as_bytes())
}

/// Seconds since the epoch, as ChatGPT writes them, to milliseconds.
fn millis(value: &Value) -> Option<UnixMillis> {
    value
        .as_f64()
        .filter(|s| s.is_finite() && *s > 0.0)
        .map(|s| UnixMillis((s * 1000.0).round() as i64))
}

/// A message worth keeping: what someone actually wrote or was shown.
struct Kept {
    node: String,
    user: bool,
    text: String,
    at: Option<UnixMillis>,
    model: Option<String>,
    message_id: Option<String>,
}

fn text_of(content: &Value) -> Option<String> {
    let parts = content["parts"].as_array();
    let text = match content["content_type"].as_str()? {
        "text" => parts?
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join("\n"),
        "multimodal_text" => parts?
            .iter()
            .filter_map(|part| match part {
                Value::String(s) => Some(s.clone()),
                Value::Object(o) => match o.get("content_type").and_then(Value::as_str) {
                    Some("image_asset_pointer") => {
                        Some("[Image not included in the import]".into())
                    }
                    Some("audio_transcription") => {
                        o.get("text").and_then(Value::as_str).map(str::to_string)
                    }
                    _ => None,
                },
                _ => None,
            })
            .filter(|s| !s.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n\n"),
        "code" => format!("```\n{}\n```", content["text"].as_str()?.trim_end()),
        // Reasoning summaries, browsing displays, custom instructions, ...
        _ => return None,
    };
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

fn kept(node_id: &str, node: &Value) -> Option<Kept> {
    let message = node.get("message").filter(|m| m.is_object())?;
    let user = match message["author"]["role"].as_str()? {
        "user" => true,
        "assistant" => false,
        _ => return None,
    };
    // Calls to tools (code, browsing) are addressed to them, not the reader.
    if message["recipient"].as_str().is_some_and(|r| r != "all") {
        return None;
    }
    let metadata = &message["metadata"];
    if metadata["is_visually_hidden_from_conversation"].as_bool() == Some(true) {
        return None;
    }
    Some(Kept {
        node: node_id.to_string(),
        user,
        text: text_of(&message["content"])?,
        at: millis(&message["create_time"]),
        model: metadata["model_slug"].as_str().map(str::to_string),
        message_id: message["id"].as_str().map(str::to_string),
    })
}

fn conversation(value: &Value, index: usize) -> Result<Option<DiscussionArchive>> {
    let foreign = value["conversation_id"]
        .as_str()
        .or_else(|| value["id"].as_str())
        .ok_or_else(|| {
            Error::Invalid(format!(
                "conversation {} in the export has no ID",
                index + 1
            ))
        })?;
    let Some(mapping) = value["mapping"].as_object() else {
        return Ok(None);
    };
    let started = millis(&value["create_time"]).unwrap_or(UnixMillis(0));

    let mut keep: HashMap<&str, Kept> = HashMap::new();
    for (id, node) in mapping {
        if let Some(k) = kept(id, node) {
            keep.insert(id.as_str(), k);
        }
    }
    if keep.is_empty() {
        return Ok(None);
    }

    // The nearest kept ancestor; the walk is bounded in case of a cycle.
    let parent_of = |id: &str| {
        let mut current = mapping.get(id)?["parent"].as_str();
        for _ in 0..mapping.len() {
            let p = current?;
            if keep.contains_key(p) {
                return Some(p);
            }
            current = mapping.get(p)?["parent"].as_str();
        }
        None
    };
    let post_id = |node: &str| derived_id("message", &format!("{foreign}:{node}"));

    // Missing times are taken from the nearest earlier message.
    let mut times: HashMap<&str, UnixMillis> = HashMap::new();
    fn time_of<'a>(
        id: &'a str,
        keep: &'a HashMap<&str, Kept>,
        parents: &dyn Fn(&str) -> Option<&'a str>,
        started: UnixMillis,
        times: &mut HashMap<&'a str, UnixMillis>,
        depth: usize,
    ) -> UnixMillis {
        if let Some(t) = times.get(id) {
            return *t;
        }
        let t = match keep[id].at {
            Some(t) => t,
            None if depth > keep.len() => started,
            None => parents(id).map_or(started, |p| {
                time_of(p, keep, parents, started, times, depth + 1)
            }),
        };
        times.insert(id, t);
        t
    }

    let you = derived_id("participant", "you");
    let mut participants = vec![ArchivedParticipant {
        id: you,
        kind: ParticipantKind::Human,
        display_name: "You".into(),
        provider: None,
        model: None,
        local_user: true,
    }];
    let mut posts = Vec::new();
    for (id, k) in &keep {
        let at = time_of(id, &keep, &parent_of, started, &mut times, 0);
        let author = if k.user {
            you
        } else {
            let slug = k.model.as_deref().unwrap_or("unknown");
            let author = derived_id("participant", &format!("model:{slug}"));
            if !participants.iter().any(|p| p.id == author) {
                participants.push(ArchivedParticipant {
                    id: author,
                    kind: ParticipantKind::Model,
                    display_name: match &k.model {
                        Some(slug) => format!("ChatGPT ({slug})"),
                        None => "ChatGPT".into(),
                    },
                    provider: Some("chatgpt".into()),
                    model: k.model.clone(),
                    local_user: false,
                });
            }
            author
        };
        posts.push((
            at,
            k.node.clone(),
            ArchivedPost {
                id: post_id(id),
                parent_id: parent_of(id).map(post_id),
                author_id: author,
                created_at: format_utc(at),
                status: PostStatus::Complete,
                body: k.text.clone(),
                edited_at: None,
                deleted_at: None,
                revisions: vec![],
                provider_metadata: (!k.user).then(|| ProviderMetadata {
                    provider: "chatgpt".into(),
                    model: k.model.clone(),
                    response_id: k.message_id.clone(),
                    request_id: None,
                    input_tokens: None,
                    output_tokens: None,
                    incomplete_reason: None,
                }),
                context_ids: vec![],
            },
        ));
    }
    // In the order written; node IDs settle ties so the result is stable.
    posts.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));

    let title = value["title"]
        .as_str()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let updated = millis(&value["update_time"]).unwrap_or(started);
    Ok(Some(DiscussionArchive {
        format: FORMAT.into(),
        exported_at: format_utc(updated),
        discussion: ArchivedDiscussion {
            id: derived_id("conversation", foreign),
            title: title
                .unwrap_or("Untitled ChatGPT conversation")
                .chars()
                .take(200)
                .collect(),
            created_at: format_utc(started),
            archived_at: value["is_archived"]
                .as_bool()
                .filter(|a| *a)
                .map(|_| format_utc(updated)),
        },
        participants,
        posts: posts.into_iter().map(|(_, _, p)| p).collect(),
    }))
}

/// Every conversation with at least one message worth keeping.
pub fn conversations(json: &str) -> Result<Vec<DiscussionArchive>> {
    let list: Vec<Value> =
        serde_json::from_str(json.trim_start_matches('\u{feff}')).map_err(|e| {
            Error::Invalid(format!(
                "this is not a ChatGPT conversations.json file: {e}"
            ))
        })?;
    let mut out = Vec::new();
    for (index, value) in list.iter().enumerate() {
        if let Some(archive) = conversation(value, index)? {
            out.push(archive);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn message(id: &str, role: &str, text: &str, at: Option<f64>) -> Value {
        json!({
            "id": format!("msg-{id}"),
            "author": { "role": role },
            "create_time": at,
            "content": { "content_type": "text", "parts": [text] },
            "recipient": "all",
            "metadata": if role == "assistant" { json!({ "model_slug": "gpt-4o" }) } else { json!({}) },
        })
    }

    /// A conversation with a hidden system root, a regenerated answer, an
    /// edited prompt, a tool call, an image, and a message without a time.
    fn export() -> Value {
        json!([{
            "title": "Datalog for the mapping engine",
            "create_time": 1_791_000_000.0,
            "update_time": 1_791_000_900.5,
            "conversation_id": "conv-1",
            "mapping": {
                "root": { "id": "root", "message": null, "parent": null, "children": ["sys"] },
                "sys": { "id": "sys", "parent": "root", "children": ["u1", "u1-edit"], "message": {
                    "id": "msg-sys", "author": { "role": "system" }, "create_time": null,
                    "content": { "content_type": "text", "parts": [""] },
                    "metadata": { "is_visually_hidden_from_conversation": true } } },
                "u1": { "id": "u1", "parent": "sys", "children": ["a1", "a1-regen"],
                        "message": message("u1", "user", "Could Datalog replace the Oracle rules?", Some(1_791_000_010.0)) },
                "a1": { "id": "a1", "parent": "u1", "children": ["tool-call"],
                        "message": message("a1", "assistant", "Yes, with stratification.", Some(1_791_000_020.25)) },
                "tool-call": { "id": "tool-call", "parent": "a1", "children": ["tool-out"], "message": {
                    "id": "msg-tc", "author": { "role": "assistant" }, "create_time": 1_791_000_030.0,
                    "recipient": "python", "content": { "content_type": "code", "text": "print(1)" }, "metadata": {} } },
                "tool-out": { "id": "tool-out", "parent": "tool-call", "children": ["a2"], "message": {
                    "id": "msg-to", "author": { "role": "tool" }, "create_time": 1_791_000_031.0,
                    "content": { "content_type": "execution_output", "text": "1" }, "metadata": {} } },
                "a2": { "id": "a2", "parent": "tool-out", "children": [],
                        "message": message("a2", "assistant", "The check printed 1.", None) },
                "a1-regen": { "id": "a1-regen", "parent": "u1", "children": [],
                        "message": message("a1-regen", "assistant", "Probably, but overrides are hard.", Some(1_791_000_040.0)) },
                "u1-edit": { "id": "u1-edit", "parent": "sys", "children": [], "message": {
                    "id": "msg-u1-edit", "author": { "role": "user" }, "create_time": 1_791_000_050.0,
                    "recipient": "all", "metadata": {},
                    "content": { "content_type": "multimodal_text", "parts": [
                        { "content_type": "image_asset_pointer", "asset_pointer": "file-service://x" },
                        "What about this diagram?" ] } } }
            }
        }, {
            "title": "Empty", "create_time": 1_791_000_000.0, "conversation_id": "conv-2",
            "mapping": { "root": { "id": "root", "message": null, "parent": null, "children": [] } }
        }])
    }

    #[test]
    fn branches_become_replies_and_tool_traffic_is_left_out() {
        let archives = conversations(&export().to_string()).unwrap();
        assert_eq!(
            archives.len(),
            1,
            "conversations with nothing to keep are skipped"
        );
        let a = &archives[0];
        assert_eq!(a.discussion.title, "Datalog for the mapping engine");
        assert!(
            DiscussionArchive::from_json(&a.to_json()).is_ok(),
            "a valid nimata/1 archive"
        );

        let body = |p: &ArchivedPost| p.body.clone();
        assert_eq!(
            a.posts.iter().map(body).collect::<Vec<_>>(),
            vec![
                "Could Datalog replace the Oracle rules?",
                "Yes, with stratification.",
                // No time of its own: it takes its parent's.
                "The check printed 1.",
                "Probably, but overrides are hard.",
                "[Image not included in the import]\n\nWhat about this diagram?",
            ]
        );
        let [question, answer, after_tool, regenerated, edited] = &a.posts[..] else {
            panic!()
        };
        // Both answers reply to the question; the edited prompt starts its
        // own thread, as it replaced the first question.
        assert_eq!(answer.parent_id, Some(question.id));
        assert_eq!(regenerated.parent_id, Some(question.id));
        assert_eq!(after_tool.parent_id, Some(answer.id));
        assert_eq!(question.parent_id, None);
        assert_eq!(edited.parent_id, None);
        assert_eq!(answer.created_at, "2026-10-03T04:00:20.250Z");
        assert_eq!(after_tool.created_at, answer.created_at);

        let you = &a.participants[0];
        assert!(you.local_user);
        assert_eq!(question.author_id, you.id);
        let gpt = a
            .participants
            .iter()
            .find(|p| p.id == answer.author_id)
            .unwrap();
        assert_eq!(gpt.display_name, "ChatGPT (gpt-4o)");
        let meta = answer.provider_metadata.as_ref().unwrap();
        assert_eq!(meta.response_id.as_deref(), Some("msg-a1"));
        assert_eq!(meta.model.as_deref(), Some("gpt-4o"));
        assert!(question.provider_metadata.is_none());
    }

    #[test]
    fn the_same_export_always_gets_the_same_ids() {
        let first = conversations(&export().to_string()).unwrap();
        let second = conversations(&export().to_string()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first[0].discussion.id, derived_id("conversation", "conv-1"));
    }

    #[test]
    fn malformed_exports_are_explained() {
        assert!(
            conversations("{}")
                .unwrap_err()
                .to_string()
                .contains("not a ChatGPT")
        );
        let no_id = json!([{ "title": "x", "mapping": {} }]).to_string();
        assert!(
            conversations(&no_id)
                .unwrap_err()
                .to_string()
                .contains("has no ID")
        );
        // A parent cycle does not hang.
        let cycle = json!([{ "conversation_id": "c", "mapping": {
            "a": { "parent": "b", "message": message("a", "user", "A", None) },
            "b": { "parent": "a", "message": null } } }]);
        assert_eq!(conversations(&cycle.to_string()).unwrap()[0].posts.len(), 1);
    }
}
