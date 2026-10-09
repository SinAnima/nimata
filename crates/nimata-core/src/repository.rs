//! The storage boundary. The rest of Nimata depends on this trait, not on
//! SQLite, so persistence can evolve without touching the domain model.

use std::collections::HashMap;

use uuid::Uuid;

use crate::attachments::{Attachment, StagedAttachment};
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
    ) -> Result<(Discussion, Post)> {
        self.start_discussion_with_attachments(title, author_id, body, &[], at)
    }

    /// Like [`Repository::start_discussion`], with files on the first post.
    /// The text may then be empty; the title falls back to the first file
    /// name.
    fn start_discussion_with_attachments(
        &mut self,
        title: &str,
        author_id: Uuid,
        body: &str,
        attachments: &[StagedAttachment],
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
    ) -> Result<Post> {
        self.add_post_with_context(discussion_id, parent_id, author_id, body, &[], at)
    }

    /// Like [`Repository::add_post`], also recording posts chosen as
    /// context. Each must be an existing, undeleted post in the same
    /// discussion; references to the post being replied to are dropped as
    /// redundant.
    fn add_post_with_context(
        &mut self,
        discussion_id: Uuid,
        parent_id: Option<Uuid>,
        author_id: Uuid,
        body: &str,
        context_ids: &[Uuid],
        at: Timestamp,
    ) -> Result<Post> {
        self.add_post_with_attachments(
            discussion_id,
            parent_id,
            author_id,
            body,
            context_ids,
            &[],
            at,
        )
    }

    /// Like [`Repository::add_post_with_context`], with attached files whose
    /// bytes are already in the blob store. A post with files may have no
    /// text.
    #[allow(clippy::too_many_arguments)]
    fn add_post_with_attachments(
        &mut self,
        discussion_id: Uuid,
        parent_id: Option<Uuid>,
        author_id: Uuid,
        body: &str,
        context_ids: &[Uuid],
        attachments: &[StagedAttachment],
        at: Timestamp,
    ) -> Result<Post>;

    fn attachment(&mut self, id: Uuid) -> Result<Attachment>;

    /// The files attached to these posts.
    fn post_attachments(&mut self, post_ids: &[Uuid]) -> Result<Vec<Attachment>>;

    /// Whether any posted attachment or draft refers to these bytes.
    fn hash_in_use(&mut self, hash: &str) -> Result<bool>;

    /// Every content hash referred to by an attachment or a draft.
    fn hashes_in_use(&mut self) -> Result<Vec<String>>;

    /// The media type recorded for `hash` when it is an image, so it can be
    /// shown.
    fn image_type(&mut self, hash: &str) -> Result<Option<String>>;

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
    ) -> Result<Option<Draft>> {
        self.save_draft_with_context(discussion_id, parent_id, body, &[], at)
    }

    /// Like [`Repository::save_draft`], keeping the context chosen for the
    /// unsent post. A draft with neither text, reply target, nor context is
    /// removed.
    fn save_draft_with_context(
        &mut self,
        discussion_id: Uuid,
        parent_id: Option<Uuid>,
        body: &str,
        context_ids: &[Uuid],
        at: UnixMillis,
    ) -> Result<Option<Draft>> {
        self.save_draft_with_attachments(discussion_id, parent_id, body, context_ids, &[], at)
    }

    /// Like [`Repository::save_draft_with_context`], keeping files added to
    /// the unsent post.
    fn save_draft_with_attachments(
        &mut self,
        discussion_id: Uuid,
        parent_id: Option<Uuid>,
        body: &str,
        context_ids: &[Uuid],
        attachments: &[StagedAttachment],
        at: UnixMillis,
    ) -> Result<Option<Draft>>;

    /// Records exactly what was sent with a request.
    fn record_sent(
        &mut self,
        generation_id: Uuid,
        sent: &crate::context::SentContext,
    ) -> Result<()>;

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

    /// Searches posts and discussion titles. Deleted posts and discussions
    /// never match; archived ones only with `in:archived`.
    fn search(
        &mut self,
        query: &crate::search::SearchQuery,
        limit: usize,
    ) -> Result<crate::search::SearchResults>;

    /// Remembers a search, most recent first.
    fn record_search(&mut self, query: &str, at: UnixMillis) -> Result<()>;

    fn recent_searches(&mut self, limit: usize) -> Result<Vec<String>>;

    fn clear_recent_searches(&mut self) -> Result<()>;

    /// Adds a discussion from an archive. Records are matched by ID, so
    /// importing the same file again changes nothing, and an older export
    /// adds only posts this device does not have. Posts already here are
    /// left as they are. A discussion deleted here stays deleted. Posts by
    /// the archive's `local_user` become the local user's.
    ///
    /// Attachments of added posts are recorded when `available` says their
    /// bytes are in the blob store; the others are counted as missing.
    fn import_discussion(
        &mut self,
        archive: &crate::archive::DiscussionArchive,
        at: UnixMillis,
        available: &dyn Fn(&str) -> bool,
    ) -> Result<crate::import::ImportOutcome>;
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
