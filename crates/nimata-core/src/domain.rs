use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::time::UnixMillis;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParticipantKind {
    Human,
    Model,
    Agent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Participant {
    pub id: Uuid,
    pub kind: ParticipantKind,
    pub display_name: String,
    pub provider: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Discussion {
    pub id: Uuid,
    pub title: String,
    pub created_at: UnixMillis,
    pub updated_at: UnixMillis,
    pub archived_at: Option<UnixMillis>,
    pub pinned_at: Option<UnixMillis>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PostStatus {
    Complete,
    Streaming,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Post {
    pub id: Uuid,
    pub discussion_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub author_id: Uuid,
    pub body: String,
    pub created_at: UnixMillis,
    /// The author's UTC offset in minutes when the post was written.
    pub tz_offset_minutes: i32,
    pub edited_at: Option<UnixMillis>,
    pub status: PostStatus,
}

/// A discussion as shown in the sidebar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscussionSummary {
    pub id: Uuid,
    pub title: String,
    pub last_activity_at: UnixMillis,
    pub post_count: usize,
    pub excerpt: String,
}

/// Everything needed to render one discussion: posts in chronological
/// order plus the participants who wrote them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscussionView {
    pub discussion: Discussion,
    pub posts: Vec<Post>,
    pub participants: Vec<Participant>,
}

impl DiscussionView {
    pub fn summary(&self) -> DiscussionSummary {
        let last = self.posts.iter().max_by_key(|p| p.created_at);
        DiscussionSummary {
            id: self.discussion.id,
            title: self.discussion.title.clone(),
            last_activity_at: last.map_or(self.discussion.updated_at, |p| p.created_at),
            post_count: self.posts.len(),
            excerpt: last.map(|p| excerpt(&p.body, 120)).unwrap_or_default(),
        }
    }
}

/// First line of `body`, cut to at most `max` characters on a char boundary.
pub fn excerpt(body: &str, max: usize) -> String {
    let line = body
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim();
    if line.chars().count() <= max {
        return line.to_string();
    }
    let cut: String = line.chars().take(max - 1).collect();
    format!("{}…", cut.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excerpt_takes_first_non_empty_line() {
        assert_eq!(excerpt("\n  Hello there\nsecond", 50), "Hello there");
    }

    #[test]
    fn excerpt_truncates_on_char_boundary() {
        assert_eq!(excerpt("νήματα threads", 4), "νήμ…");
    }

    #[test]
    fn post_serializes_in_camel_case_with_millis() {
        let post = Post {
            id: Uuid::nil(),
            discussion_id: Uuid::nil(),
            parent_id: None,
            author_id: Uuid::nil(),
            body: "x".into(),
            created_at: UnixMillis(1_700_000_000_000),
            tz_offset_minutes: -240,
            edited_at: None,
            status: PostStatus::Complete,
        };
        let json = serde_json::to_value(&post).unwrap();
        assert_eq!(json["createdAt"], 1_700_000_000_000_i64);
        assert_eq!(json["tzOffsetMinutes"], -240);
        assert_eq!(json["status"], "complete");
    }
}
