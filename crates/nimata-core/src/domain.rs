use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{Error, Result};
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
    /// Set when the post was deleted. The post stays as a tombstone so the
    /// thread keeps its shape; its body is erased.
    pub deleted_at: Option<UnixMillis>,
    pub status: PostStatus,
}

impl Post {
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}

/// An earlier version of an edited post: the text it had from `written_at`
/// until an edit replaced it at `replaced_at`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Revision {
    pub post_id: Uuid,
    pub body: String,
    pub written_at: UnixMillis,
    pub replaced_at: UnixMillis,
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
/// order, the participants who wrote them, and any unsent draft.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscussionView {
    pub discussion: Discussion,
    pub posts: Vec<Post>,
    pub participants: Vec<Participant>,
    pub draft: Option<Draft>,
}

/// An unsent post, kept per discussion so it survives navigation and restarts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Draft {
    pub discussion_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub body: String,
    pub updated_at: UnixMillis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiscussionFilter {
    Active,
    Archived,
}

pub const MAX_TITLE_CHARS: usize = 200;

/// Trims a title and checks it is usable.
pub fn clean_title(title: &str) -> Result<String> {
    let title = title.trim();
    if title.is_empty() {
        return Err(Error::Invalid("a discussion needs a title".into()));
    }
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(Error::Invalid(format!(
            "titles are limited to {MAX_TITLE_CHARS} characters"
        )));
    }
    Ok(title.to_string())
}

/// Trims surrounding blank lines and checks the post has content. Inner
/// whitespace is the author's and is kept as written.
pub fn clean_body(body: &str) -> Result<String> {
    let body = body
        .trim_matches(|c: char| c == '\n' || c == '\r')
        .trim_end();
    if body.trim().is_empty() {
        return Err(Error::Invalid("a post cannot be empty".into()));
    }
    Ok(body.to_string())
}

/// The title used when a discussion is started without one.
pub fn title_from_body(body: &str) -> String {
    excerpt(body, 80)
}

pub fn clean_display_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(Error::Invalid("names must be 1 to 80 characters".into()));
    }
    Ok(name.to_string())
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
    fn titles_are_trimmed_and_must_not_be_empty() {
        assert_eq!(clean_title("  Datalog  ").unwrap(), "Datalog");
        assert!(clean_title("   ").is_err());
        assert!(clean_title(&"x".repeat(MAX_TITLE_CHARS + 1)).is_err());
    }

    #[test]
    fn bodies_keep_inner_whitespace_but_lose_surrounding_blank_lines() {
        assert_eq!(
            clean_body("\n\n  indented\n\n  more  \n\n").unwrap(),
            "  indented\n\n  more"
        );
        assert!(clean_body(" \n \t\n").is_err());
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
            deleted_at: None,
            status: PostStatus::Complete,
        };
        let json = serde_json::to_value(&post).unwrap();
        assert_eq!(json["createdAt"], 1_700_000_000_000_i64);
        assert_eq!(json["tzOffsetMinutes"], -240);
        assert_eq!(json["status"], "complete");
    }
}
