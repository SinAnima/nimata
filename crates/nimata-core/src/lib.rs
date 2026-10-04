//! Nimata core: the domain model and discussion logic, independent of any UI.

pub mod domain;
pub mod fixtures;
pub mod time;

pub use domain::{
    Discussion, DiscussionSummary, DiscussionView, Participant, ParticipantKind, Post, PostStatus,
};
