//! The storage boundary. The rest of Nimata depends on this trait, not on
//! SQLite, so persistence can evolve without touching the domain model.

use std::collections::HashMap;

use uuid::Uuid;

use crate::domain::{
    Discussion, DiscussionFilter, DiscussionSummary, DiscussionView, Draft, Generation,
    GenerationStatus, ModelParticipant, Participant, Post, ProviderConfig, ProviderKind,
    ProviderMetadata, Revision,
};
use crate::error::Result;
use crate::time::{Timestamp, UnixMillis};

pub trait Repository {
    /// The participant representing the person using this device, created
    /// on first use.
    fn local_user(&mut self) -> Result<Participant>;

    fn rename_participant(&mut self, id: Uuid, display_name: &str) -> Result<Participant>;

    /// Creates a discussion together with its first post, atomically. An
    /// empty title is derived from the first line of the body.
    fn start_discussion(
        &mut self,
        title: &str,
        author_id: Uuid,
        body: &str,
        at: Timestamp,
    ) -> Result<(Discussion, Post)>;

    fn rename_discussion(&mut self, id: Uuid, title: &str, at: UnixMillis) -> Result<Discussion>;

    fn set_archived(&mut self, id: Uuid, archived: bool, at: UnixMillis) -> Result<Discussion>;

    /// Discussions ordered by latest activity, newest first.
    fn list_discussions(&mut self, filter: DiscussionFilter) -> Result<Vec<DiscussionSummary>>;

    fn get_discussion(&mut self, id: Uuid) -> Result<DiscussionView>;

    /// Appends a reply. Every post after a discussion's first replies to an
    /// earlier post in the same discussion; a new topic is a new discussion.
    /// Clears the discussion's draft in the same transaction.
    fn add_post(
        &mut self,
        discussion_id: Uuid,
        parent_id: Option<Uuid>,
        author_id: Uuid,
        body: &str,
        at: Timestamp,
    ) -> Result<Post>;

    /// Replaces a post's text, keeping the previous text as a revision. Only
    /// the author can edit, and deleted posts cannot be edited. Returns the
    /// post unchanged if the text is the same.
    fn edit_post(
        &mut self,
        post_id: Uuid,
        editor_id: Uuid,
        body: &str,
        at: UnixMillis,
    ) -> Result<Post>;

    /// Earlier versions of a post, oldest first.
    fn post_revisions(&mut self, post_id: Uuid) -> Result<Vec<Revision>>;

    /// Earlier versions of every edited post in a discussion.
    fn discussion_revisions(&mut self, discussion_id: Uuid)
    -> Result<HashMap<Uuid, Vec<Revision>>>;

    /// Deletes a post by its author. The post remains as a tombstone, so
    /// replies keep their place in the thread; its text and revisions are
    /// erased.
    fn delete_post(&mut self, post_id: Uuid, by: Uuid, at: UnixMillis) -> Result<Post>;

    /// Deletes a discussion. It remains as a tombstone with its title, posts,
    /// revisions, and draft erased.
    fn delete_discussion(&mut self, id: Uuid, at: UnixMillis) -> Result<()>;

    /// Saves or replaces the draft. An empty body with no reply target
    /// removes it.
    fn save_draft(
        &mut self,
        discussion_id: Uuid,
        parent_id: Option<Uuid>,
        body: &str,
        at: UnixMillis,
    ) -> Result<Option<Draft>>;

    /// The provider of `kind`, created with the standard endpoint on first
    /// use. Changing its endpoint later does not make it a different provider.
    fn standard_provider(&mut self, kind: ProviderKind, at: UnixMillis) -> Result<ProviderConfig>;

    fn provider(&mut self, id: Uuid) -> Result<ProviderConfig>;

    /// Points a provider at a different endpoint; `None` restores the
    /// provider's standard one.
    fn set_provider_base_url(
        &mut self,
        id: Uuid,
        base_url: Option<&str>,
        at: UnixMillis,
    ) -> Result<ProviderConfig>;

    /// Every provider: the standard ones first, then added connections in the
    /// order they were added.
    fn providers(&mut self) -> Result<Vec<ProviderConfig>>;

    /// Adds a connection of a kind that allows several (OpenAI-compatible).
    fn add_provider(
        &mut self,
        kind: ProviderKind,
        display_name: &str,
        base_url: &str,
        at: UnixMillis,
    ) -> Result<ProviderConfig>;

    fn rename_provider(
        &mut self,
        id: Uuid,
        display_name: &str,
        at: UnixMillis,
    ) -> Result<ProviderConfig>;

    /// Removes an added connection whose models have written nothing. A
    /// connection whose models wrote posts stays, so no post loses its
    /// author; its models can be turned off instead.
    fn remove_provider(&mut self, id: Uuid) -> Result<()>;

    /// Model participants, all providers, enabled or not, by display name.
    fn model_participants(&mut self) -> Result<Vec<ModelParticipant>>;

    /// Turns a provider's model into a participant (or updates it). Posts
    /// already written by the model keep their author either way.
    fn set_model(
        &mut self,
        provider_id: Uuid,
        model: &str,
        display_name: &str,
        enabled: bool,
    ) -> Result<ModelParticipant>;

    /// Replaces a model's aliases. Each alias must be unused by every other
    /// model, including their automatic aliases.
    fn set_model_aliases(
        &mut self,
        participant_id: Uuid,
        aliases: &[String],
    ) -> Result<ModelParticipant>;

    /// The model that answers when nothing else decides, if one is chosen.
    fn default_model(&mut self) -> Result<Option<Uuid>>;

    fn set_default_model(&mut self, participant_id: Option<Uuid>) -> Result<()>;

    /// Starts a model reply to `parent_id`: creates the reply as a streaming
    /// post and records the request, including which posts are sent.
    fn begin_generation(
        &mut self,
        discussion_id: Uuid,
        parent_id: Uuid,
        participant_id: Uuid,
        context_post_ids: &[Uuid],
        at: Timestamp,
    ) -> Result<(Post, Generation)>;

    /// Records progress: the status and the text received so far.
    fn update_generation(
        &mut self,
        generation_id: Uuid,
        status: GenerationStatus,
        body: &str,
    ) -> Result<()>;

    /// Finishes a reply as complete, failed, or cancelled. Text received
    /// before a failure or cancellation is kept.
    fn finish_generation(
        &mut self,
        generation_id: Uuid,
        outcome: GenerationOutcome,
        at: UnixMillis,
    ) -> Result<Post>;

    fn generation_for_post(&mut self, post_id: Uuid) -> Result<Option<Generation>>;

    /// The discussion a post belongs to, and the post it replies to.
    fn discussion_of_post(&mut self, post_id: Uuid) -> Result<(Uuid, Option<Uuid>)>;

    /// Marks requests that were still running when Nimata last stopped as
    /// failed, keeping any text they had received. Returns how many.
    fn recover_interrupted(&mut self, at: UnixMillis) -> Result<usize>;
}

/// How a model reply ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationOutcome {
    pub status: GenerationStatus,
    pub body: String,
    pub metadata: Option<ProviderMetadata>,
    pub error: Option<String>,
}

pub const INTERRUPTED: &str = "Nimata was closed before this reply was finished.";
