//! The storage boundary. The rest of Nimata depends on this trait, not on
//! SQLite, so persistence can evolve without touching the domain model.

use uuid::Uuid;

use crate::domain::{
    Discussion, DiscussionFilter, DiscussionSummary, DiscussionView, Draft, Participant, Post,
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

    fn rename_discussion(&mut self, id: Uuid, title: &str) -> Result<Discussion>;

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
