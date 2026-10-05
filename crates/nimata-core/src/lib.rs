//! Nimata core: the domain model and discussion logic, independent of any UI.

pub mod domain;
pub mod error;
pub mod repository;
pub mod sqlite;
pub mod time;

pub use domain::{
    Discussion, DiscussionFilter, DiscussionSummary, DiscussionView, Draft, Participant,
    ParticipantKind, Post, PostStatus,
};
pub use error::{Error, Result};
pub use repository::Repository;
pub use sqlite::SqliteRepository;
pub use time::{Timestamp, UnixMillis};
