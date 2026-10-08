//! The canonical JSON representation of a discussion (`nimata/1`).
//!
//! It is independent of the database layout and meant to stay intelligible
//! without Nimata. Times are RFC 3339 strings with millisecond precision; a
//! post's `createdAt` carries the author's own UTC offset. See
//! docs/archive-format.md.

use std::collections::HashMap;

use chrono::{DateTime, FixedOffset, SecondsFormat};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{DiscussionView, ParticipantKind, PostStatus, ProviderMetadata, Revision};
use crate::error::{Error, Result};
use crate::time::UnixMillis;

pub const FORMAT: &str = "nimata/1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscussionArchive {
    pub format: String,
    pub exported_at: String,
    pub discussion: ArchivedDiscussion,
    pub participants: Vec<ArchivedParticipant>,
    /// In the order they were written.
    pub posts: Vec<ArchivedPost>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedDiscussion {
    pub id: Uuid,
    pub title: String,
    pub created_at: String,
    pub archived_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedParticipant {
    pub id: Uuid,
    pub kind: ParticipantKind,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub model: Option<String>,
    /// True for the person who exported the file. On import, their posts
    /// become the importing person's own.
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub local_user: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedPost {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub author_id: Uuid,
    /// When written, in the author's UTC offset.
    pub created_at: String,
    pub status: PostStatus,
    /// Empty for a deleted post.
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub edited_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub deleted_at: Option<String>,
    /// Earlier versions, oldest first.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub revisions: Vec<ArchivedRevision>,
    /// For model replies: which provider and model version answered, the
    /// provider's own IDs, and token usage.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub provider_metadata: Option<ProviderMetadata>,
    /// Other posts the author chose as context, besides the one replied to.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub context_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedRevision {
    pub body: String,
    pub written_at: String,
    pub replaced_at: String,
}

/// Formats an instant as RFC 3339 in the given UTC offset, with
/// milliseconds, e.g. `2026-10-04T12:41:07.123-04:00`.
pub fn format_time(at: UnixMillis, offset_minutes: i32) -> String {
    let offset = FixedOffset::east_opt(offset_minutes * 60)
        .unwrap_or_else(|| FixedOffset::east_opt(0).expect("zero offset is valid"));
    DateTime::from_timestamp_millis(at.0)
        .expect("stored times are within chrono's range")
        .with_timezone(&offset)
        .to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub fn format_utc(at: UnixMillis) -> String {
    format_time(at, 0)
}

/// Parses an RFC 3339 time back into the instant and its UTC offset.
pub fn parse_time(text: &str) -> Result<(UnixMillis, i32)> {
    let parsed = DateTime::parse_from_rfc3339(text)
        .map_err(|e| Error::Invalid(format!("invalid time {text:?}: {e}")))?;
    Ok((
        UnixMillis(parsed.timestamp_millis()),
        parsed.offset().local_minus_utc() / 60,
    ))
}

impl DiscussionArchive {
    /// Builds the archive for one discussion. `revisions` holds the earlier
    /// versions of edited posts, keyed by post ID; `local_user` is the
    /// person exporting.
    pub fn new(
        view: &DiscussionView,
        revisions: &HashMap<Uuid, Vec<Revision>>,
        local_user: Uuid,
        exported_at: UnixMillis,
    ) -> Self {
        let d = &view.discussion;
        Self {
            format: FORMAT.to_string(),
            exported_at: format_utc(exported_at),
            discussion: ArchivedDiscussion {
                id: d.id,
                title: d.title.clone(),
                created_at: format_utc(d.created_at),
                archived_at: d.archived_at.map(format_utc),
            },
            participants: view
                .participants
                .iter()
                .map(|p| ArchivedParticipant {
                    id: p.id,
                    kind: p.kind,
                    display_name: p.display_name.clone(),
                    provider: p.provider.clone(),
                    model: p.model.clone(),
                    local_user: p.id == local_user,
                })
                .collect(),
            posts: view
                .posts
                .iter()
                .map(|p| ArchivedPost {
                    id: p.id,
                    parent_id: p.parent_id,
                    author_id: p.author_id,
                    created_at: format_time(p.created_at, p.tz_offset_minutes),
                    status: p.status,
                    body: p.body.clone(),
                    edited_at: p.edited_at.map(format_utc),
                    deleted_at: p.deleted_at.map(format_utc),
                    revisions: revisions
                        .get(&p.id)
                        .into_iter()
                        .flatten()
                        .map(|r| ArchivedRevision {
                            body: r.body.clone(),
                            written_at: format_utc(r.written_at),
                            replaced_at: format_utc(r.replaced_at),
                        })
                        .collect(),
                    provider_metadata: p.provider_metadata.clone(),
                    context_ids: p.context_ids.clone(),
                })
                .collect(),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("archive types always serialize")
    }

    /// Reads an archive, checking the format and that every reply's parent
    /// and every author is present.
    pub fn from_json(json: &str) -> Result<Self> {
        let archive: Self = serde_json::from_str(json)
            .map_err(|e| Error::Invalid(format!("not a Nimata discussion archive: {e}")))?;
        if archive.format != FORMAT {
            return Err(Error::Invalid(format!(
                "unsupported archive format {:?}; this version reads {FORMAT:?}",
                archive.format
            )));
        }
        let post_ids: Vec<Uuid> = archive.posts.iter().map(|p| p.id).collect();
        for post in &archive.posts {
            if post
                .parent_id
                .is_some_and(|parent| !post_ids.contains(&parent))
            {
                return Err(Error::Invalid(format!(
                    "post {} replies to a missing post",
                    post.id
                )));
            }
            if post.context_ids.iter().any(|id| !post_ids.contains(id)) {
                return Err(Error::Invalid(format!(
                    "post {} refers to a missing post as context",
                    post.id
                )));
            }
            if !archive.participants.iter().any(|a| a.id == post.author_id) {
                return Err(Error::Invalid(format!(
                    "post {} has an unknown author",
                    post.id
                )));
            }
            parse_time(&post.created_at)?;
        }
        Ok(archive)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Discussion, Participant, Post};

    fn sample() -> (DiscussionView, HashMap<Uuid, Vec<Revision>>) {
        let me = Participant {
            id: Uuid::now_v7(),
            kind: ParticipantKind::Human,
            display_name: "Thanos".into(),
            provider: None,
            model: None,
        };
        let discussion = Discussion {
            id: Uuid::now_v7(),
            title: "Should Nimata use CouchDB?".into(),
            created_at: UnixMillis(1_791_132_067_123),
            updated_at: UnixMillis(1_791_132_187_123),
            archived_at: None,
            pinned_at: None,
        };
        let post = |parent: Option<Uuid>,
                    body: &str,
                    at: i64,
                    offset: i32,
                    edited: Option<i64>,
                    deleted: Option<i64>| Post {
            id: Uuid::now_v7(),
            discussion_id: discussion.id,
            parent_id: parent,
            author_id: me.id,
            body: body.into(),
            created_at: UnixMillis(at),
            tz_offset_minutes: offset,
            edited_at: edited.map(UnixMillis),
            deleted_at: deleted.map(UnixMillis),
            status: PostStatus::Complete,
            provider_metadata: None,
            context_ids: vec![],
        };
        let root = post(
            None,
            "Should Nimata use CouchDB?",
            1_791_132_067_123,
            -240,
            None,
            None,
        );
        let edited = post(
            Some(root.id),
            "The attraction is replication.",
            1_791_132_127_123,
            180,
            Some(1_791_132_160_000),
            None,
        );
        let deleted = post(
            Some(root.id),
            "",
            1_791_132_187_123,
            0,
            None,
            Some(1_791_132_190_000),
        );
        let revisions = HashMap::from([(
            edited.id,
            vec![Revision {
                post_id: edited.id,
                body: "The attraction is replicaton.".into(),
                written_at: edited.created_at,
                replaced_at: UnixMillis(1_791_132_160_000),
            }],
        )]);
        let view = DiscussionView {
            discussion,
            posts: vec![root, edited, deleted],
            participants: vec![me],
            draft: None,
        };
        (view, revisions)
    }

    #[test]
    fn times_keep_the_author_offset_and_millisecond_precision() {
        assert_eq!(
            format_time(UnixMillis(1_791_132_067_123), -240),
            "2026-10-04T12:41:07.123-04:00"
        );
        assert_eq!(
            format_utc(UnixMillis(1_791_132_067_123)),
            "2026-10-04T16:41:07.123Z"
        );
        assert_eq!(
            parse_time("2026-10-04T12:41:07.123-04:00").unwrap(),
            (UnixMillis(1_791_132_067_123), -240)
        );
    }

    #[test]
    fn every_post_time_round_trips_through_json() {
        let (view, revisions) = sample();
        let json =
            DiscussionArchive::new(&view, &revisions, view.participants[0].id, UnixMillis(0))
                .to_json();
        let back = DiscussionArchive::from_json(&json).unwrap();
        for (original, archived) in view.posts.iter().zip(&back.posts) {
            assert_eq!(
                parse_time(&archived.created_at).unwrap(),
                (original.created_at, original.tz_offset_minutes)
            );
            assert_eq!(archived.parent_id, original.parent_id);
        }
        assert_eq!(
            back,
            DiscussionArchive::new(&view, &revisions, view.participants[0].id, UnixMillis(0))
        );
    }

    #[test]
    fn the_json_is_readable_and_omits_empty_fields() {
        let (view, revisions) = sample();
        let json: serde_json::Value = serde_json::from_str(
            &DiscussionArchive::new(&view, &revisions, view.participants[0].id, UnixMillis(0))
                .to_json(),
        )
        .unwrap();
        assert_eq!(json["format"], "nimata/1");
        assert_eq!(
            json["posts"][0]["createdAt"],
            "2026-10-04T12:41:07.123-04:00"
        );
        assert!(json["posts"][0].get("revisions").is_none());
        assert_eq!(
            json["posts"][1]["revisions"][0]["body"],
            "The attraction is replicaton."
        );
        assert_eq!(json["posts"][2]["body"], "");
        assert!(json["posts"][2]["deletedAt"].is_string());
        assert!(json["participants"][0].get("provider").is_none());
        assert_eq!(json["participants"][0]["localUser"], true);
    }

    #[test]
    fn archives_with_broken_references_are_rejected() {
        let (view, revisions) = sample();
        let mut archive =
            DiscussionArchive::new(&view, &revisions, view.participants[0].id, UnixMillis(0));
        archive.posts[1].parent_id = Some(Uuid::now_v7());
        assert!(DiscussionArchive::from_json(&archive.to_json()).is_err());

        let mut archive =
            DiscussionArchive::new(&view, &revisions, view.participants[0].id, UnixMillis(0));
        archive.format = "nimata/99".into();
        let error = DiscussionArchive::from_json(&archive.to_json()).unwrap_err();
        assert!(error.to_string().contains("unsupported archive format"));

        assert!(DiscussionArchive::from_json("{\"hello\": 1}").is_err());
    }
}
