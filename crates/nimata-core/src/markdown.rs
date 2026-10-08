//! A discussion as Markdown: the posts in the order they were written, each
//! with its author, exact time, and a link to the post it replies to.
//!
//! Made from the canonical archive, so it shows exactly what a JSON export
//! contains. It is for reading and sharing; Nimata cannot import it.

use std::collections::HashMap;

use chrono::DateTime;
use uuid::Uuid;

use crate::archive::{ArchivedPost, DiscussionArchive};
use crate::domain::PostStatus;

/// A post's author time, e.g. `2026-10-04 12:41:07 UTC−04:00`.
fn wall_clock(time: &str) -> String {
    let Ok(parsed) = DateTime::parse_from_rfc3339(time) else {
        return time.to_string();
    };
    let offset = parsed.offset().local_minus_utc() / 60;
    let zone = if offset == 0 {
        "UTC".to_string()
    } else {
        let sign = if offset < 0 { '−' } else { '+' };
        format!("UTC{sign}{:02}:{:02}", offset.abs() / 60, offset.abs() % 60)
    };
    format!("{} {zone}", parsed.format("%Y-%m-%d %H:%M:%S"))
}

/// The anchor a post's heading gets, and links point to.
fn anchor(id: Uuid) -> String {
    format!("post-{id}")
}

pub fn discussion_markdown(archive: &DiscussionArchive) -> String {
    let names: HashMap<Uuid, String> = archive
        .participants
        .iter()
        .map(|p| (p.id, p.display_name.clone()))
        .collect();
    let name = |id: &Uuid| names.get(id).cloned().unwrap_or_else(|| "Someone".into());
    let posts: HashMap<Uuid, &ArchivedPost> = archive.posts.iter().map(|p| (p.id, p)).collect();
    let link = |id: &Uuid| match posts.get(id) {
        Some(p) => format!(
            "[{}, {}](#{})",
            name(&p.author_id),
            wall_clock(&p.created_at),
            anchor(*id)
        ),
        None => "an earlier post".into(),
    };

    let mut ordered: Vec<&ArchivedPost> = archive.posts.iter().collect();
    ordered.sort_by_key(|p| {
        DateTime::parse_from_rfc3339(&p.created_at)
            .map(|t| t.timestamp_millis())
            .ok()
    });

    let mut out = format!("# {}\n\n", archive.discussion.title.trim());
    let count = archive.posts.len();
    out.push_str(&format!(
        "Exported from Nimata at {}. {count} {}, in the order they were written; \
         times are each author's own.\n",
        wall_clock(&archive.exported_at),
        if count == 1 { "post" } else { "posts" },
    ));

    for post in ordered {
        out.push_str("\n---\n\n");
        out.push_str(&format!("<a id=\"{}\"></a>\n", anchor(post.id)));
        let author = archive.participants.iter().find(|p| p.id == post.author_id);
        let model = post
            .provider_metadata
            .as_ref()
            .and_then(|m| m.model.clone())
            .or_else(|| author.and_then(|a| a.model.clone()));
        out.push_str(&format!("**{}**", name(&post.author_id)));
        if let Some(model) = model {
            out.push_str(&format!(" ({model})"));
        }
        out.push_str(&format!(", {}", wall_clock(&post.created_at)));
        if let Some(parent) = &post.parent_id {
            out.push_str(&format!("  \nReplying to {}", link(parent)));
        }
        if !post.context_ids.is_empty() {
            let refs: Vec<String> = post.context_ids.iter().map(link).collect();
            out.push_str(&format!("  \nAlso considering {}", refs.join(", ")));
        }
        out.push_str("\n\n");

        if post.deleted_at.is_some() {
            out.push_str("_This post was deleted._\n");
            continue;
        }
        if !post.body.trim().is_empty() {
            out.push_str(post.body.trim_end());
            out.push('\n');
        }
        match post.status {
            PostStatus::Failed => out.push_str("\n_The reply failed before it finished._\n"),
            PostStatus::Cancelled => {
                out.push_str("\n_The reply was stopped before it finished._\n")
            }
            PostStatus::Streaming => out.push_str("\n_The reply was still being written._\n"),
            PostStatus::Complete => {}
        }
        if let Some(edited) = &post.edited_at {
            out.push_str(&format!("\n_Edited {}._\n", wall_clock(edited)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::{ArchivedDiscussion, ArchivedParticipant, FORMAT};
    use crate::domain::{ParticipantKind, ProviderMetadata};

    fn participant(name: &str, kind: ParticipantKind, model: Option<&str>) -> ArchivedParticipant {
        ArchivedParticipant {
            id: Uuid::now_v7(),
            kind,
            display_name: name.into(),
            provider: None,
            model: model.map(str::to_string),
            local_user: false,
        }
    }

    fn post(author: Uuid, parent: Option<Uuid>, at: &str, body: &str) -> ArchivedPost {
        ArchivedPost {
            id: Uuid::now_v7(),
            parent_id: parent,
            author_id: author,
            created_at: at.into(),
            status: PostStatus::Complete,
            body: body.into(),
            edited_at: None,
            deleted_at: None,
            revisions: vec![],
            provider_metadata: None,
            context_ids: vec![],
        }
    }

    #[test]
    fn posts_are_in_written_order_with_reply_links_and_exact_times() {
        let me = participant("Thanos", ParticipantKind::Human, None);
        let claude = participant(
            "Claude Opus",
            ParticipantKind::Model,
            Some("claude-opus-5-5"),
        );
        let question = post(
            me.id,
            None,
            "2026-10-04T12:41:07.123-04:00",
            "Should Nimata use CouchDB?",
        );
        let mut answer = post(
            claude.id,
            Some(question.id),
            "2026-10-04T16:42:00.000Z",
            "Probably not.\n\nSQLite is enough.",
        );
        answer.provider_metadata = Some(ProviderMetadata {
            provider: "anthropic".into(),
            model: Some("claude-opus-5-5-20260901".into()),
            response_id: None,
            request_id: None,
            input_tokens: None,
            output_tokens: None,
            incomplete_reason: None,
        });
        let mut gone = post(me.id, Some(answer.id), "2026-10-04T19:45:00.000+03:00", "");
        gone.deleted_at = Some("2026-10-04T16:50:00.000Z".into());
        let mut edited = post(
            me.id,
            Some(question.id),
            "2026-10-04T16:43:00.000Z",
            "Fine.",
        );
        edited.edited_at = Some("2026-10-04T16:44:00.000Z".into());
        edited.context_ids = vec![answer.id];

        let archive = DiscussionArchive {
            format: FORMAT.into(),
            exported_at: "2026-10-07T10:00:00.000Z".into(),
            discussion: ArchivedDiscussion {
                id: Uuid::now_v7(),
                title: "CouchDB?".into(),
                created_at: "2026-10-04T16:41:07.123Z".into(),
                archived_at: None,
            },
            participants: vec![me, claude],
            // Out of order on purpose: the export orders by instant.
            posts: vec![
                gone.clone(),
                question.clone(),
                edited.clone(),
                answer.clone(),
            ],
        };
        let md = discussion_markdown(&archive);

        let expected = format!(
            "# CouchDB?\n\n\
             Exported from Nimata at 2026-10-07 10:00:00 UTC. 4 posts, in the order they were written; times are each author's own.\n\
             \n---\n\n<a id=\"post-{q}\"></a>\n**Thanos**, 2026-10-04 12:41:07 UTC−04:00\n\nShould Nimata use CouchDB?\n\
             \n---\n\n<a id=\"post-{a}\"></a>\n**Claude Opus** (claude-opus-5-5-20260901), 2026-10-04 16:42:00 UTC  \nReplying to [Thanos, 2026-10-04 12:41:07 UTC−04:00](#post-{q})\n\nProbably not.\n\nSQLite is enough.\n\
             \n---\n\n<a id=\"post-{e}\"></a>\n**Thanos**, 2026-10-04 16:43:00 UTC  \nReplying to [Thanos, 2026-10-04 12:41:07 UTC−04:00](#post-{q})  \nAlso considering [Claude Opus, 2026-10-04 16:42:00 UTC](#post-{a})\n\nFine.\n\n_Edited 2026-10-04 16:44:00 UTC._\n\
             \n---\n\n<a id=\"post-{g}\"></a>\n**Thanos**, 2026-10-04 19:45:00 UTC+03:00  \nReplying to [Claude Opus, 2026-10-04 16:42:00 UTC](#post-{a})\n\n_This post was deleted._\n",
            q = question.id,
            a = answer.id,
            e = edited.id,
            g = gone.id,
        );
        assert_eq!(md, expected);
    }
}
