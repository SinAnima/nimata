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
    /// For posts written by a model: which provider and model version
    /// answered, the provider's own IDs, and token usage. Subordinate
    /// metadata, never part of Nimata's identity for the post.
    #[serde(default)]
    pub provider_metadata: Option<ProviderMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderMetadata {
    pub provider: String,
    /// The exact model version that answered, as reported by the provider.
    pub model: Option<String>,
    pub response_id: Option<String>,
    pub request_id: Option<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    /// Set when the reply stopped early, e.g. at the output token limit.
    pub incomplete_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    #[serde(rename = "openai")]
    OpenAi,
    Anthropic,
    #[serde(rename = "openai_compatible")]
    OpenAiCompatible,
}

impl ProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
            Self::OpenAiCompatible => "openai_compatible",
        }
    }
}

/// Non-secret settings for one provider connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    pub id: Uuid,
    pub kind: ProviderKind,
    pub display_name: String,
    /// `None` means the provider's standard endpoint.
    pub base_url: Option<String>,
}

/// A model that can be asked to reply, as listed in Settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelParticipant {
    pub participant: Participant,
    pub provider_id: Uuid,
    pub enabled: bool,
    /// Names the model can be mentioned by, without the `@`. Every model can
    /// also be mentioned by [`automatic_alias`] of its display name.
    pub aliases: Vec<String>,
}

pub const MAX_ALIAS_CHARS: usize = 32;

/// Normalises an alias: lowercase, without a leading `@`. Letters, digits,
/// `.`, `_`, and `-` are allowed; it must start with a letter or digit.
pub fn clean_alias(alias: &str) -> Result<String> {
    let alias = alias.trim().trim_start_matches('@').to_lowercase();
    let valid_chars = alias
        .chars()
        .all(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-'));
    let starts_well = alias.chars().next().is_some_and(char::is_alphanumeric);
    if alias.is_empty() || alias.chars().count() > MAX_ALIAS_CHARS || !valid_chars || !starts_well {
        return Err(Error::Invalid(format!(
            "\"{alias}\" cannot be an alias: use up to {MAX_ALIAS_CHARS} letters, digits, dots, dashes, or underscores, starting with a letter or digit"
        )));
    }
    Ok(alias)
}

/// The alias every model has without setting one: its display name in
/// lowercase with only letters and digits, e.g. "GPT-6-sol" -> "gpt6sol".
pub fn automatic_alias(display_name: &str) -> String {
    display_name
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GenerationStatus {
    Queued,
    Sending,
    Streaming,
    Complete,
    Failed,
    Cancelled,
}

impl GenerationStatus {
    pub fn is_finished(self) -> bool {
        matches!(self, Self::Complete | Self::Failed | Self::Cancelled)
    }
}

/// The record of one request to a model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Generation {
    pub id: Uuid,
    pub post_id: Uuid,
    pub participant_id: Uuid,
    pub status: GenerationStatus,
    pub error: Option<String>,
    /// Exactly which posts were sent to the model, in order.
    pub context_post_ids: Vec<Uuid>,
    pub started_at: UnixMillis,
    pub finished_at: Option<UnixMillis>,
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
    fn aliases_are_normalised_and_checked() {
        assert_eq!(clean_alias(" @Review ").unwrap(), "review");
        assert_eq!(clean_alias("gpt-6.sol_2").unwrap(), "gpt-6.sol_2");
        for bad in [
            "",
            "@",
            "two words",
            "-dash",
            "x".repeat(33).as_str(),
            "re@view",
        ] {
            assert!(clean_alias(bad).is_err(), "{bad:?}");
        }
        assert_eq!(automatic_alias("GPT-6-sol"), "gpt6sol");
        assert_eq!(automatic_alias("Claude Sonnet"), "claudesonnet");
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
            provider_metadata: None,
        };
        let json = serde_json::to_value(&post).unwrap();
        assert_eq!(json["createdAt"], 1_700_000_000_000_i64);
        assert_eq!(json["tzOffsetMinutes"], -240);
        assert_eq!(json["status"], "complete");
    }
}
