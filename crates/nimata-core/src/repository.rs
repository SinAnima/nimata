//! The storage boundary. The rest of Nimata depends on this trait, not on
//! SQLite, so persistence can evolve without touching the domain model.

use std::collections::HashMap;

use uuid::Uuid;

use crate::domain::{
    Discussion, DiscussionFilter, DiscussionSummary, DiscussionView, Draft, Participant, Post,
    Revision,
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

    /// Appends a post. `parent_id`, when present, must be a post in the same
    /// discussion. Clears the discussion's draft in the same transaction.
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
}
