//! Context references: posts chosen as context, separate from the reply.

use nimata_core::{DiscussionFilter, Error, Repository, SqliteRepository, Timestamp, UnixMillis};
use uuid::Uuid;

fn at(minutes: i64) -> Timestamp {
    Timestamp {
        at: UnixMillis(1_791_132_067_123 + minutes * 60_000),
        offset_minutes: 0,
    }
}

struct Thread {
    repo: SqliteRepository,
    me: Uuid,
    discussion: Uuid,
    question: Uuid,
    left: Uuid,
    right: Uuid,
}

/// A question with two sibling answers.
fn thread() -> Thread {
    let mut repo = SqliteRepository::open_in_memory().unwrap();
    let me = repo.local_user().unwrap().id;
    let (d, q) = repo
        .start_discussion("", me, "Should Nimata use CouchDB?", at(0))
        .unwrap();
    let left = repo
        .add_post(d.id, Some(q.id), me, "Yes: replication.", at(1))
        .unwrap()
        .id;
    let right = repo
        .add_post(d.id, Some(q.id), me, "No: mobile.", at(2))
        .unwrap()
        .id;
    Thread {
        repo,
        me,
        discussion: d.id,
        question: q.id,
        left,
        right,
    }
}

#[test]
fn a_post_keeps_its_context_separately_from_its_reply() {
    let mut t = thread();
    let post = t
        .repo
        .add_post_with_context(
            t.discussion,
            Some(t.left),
            t.me,
            "Weighing both.",
            &[t.right],
            at(3),
        )
        .unwrap();
    assert_eq!(post.parent_id, Some(t.left));
    assert_eq!(post.context_ids, vec![t.right]);

    let view = t.repo.get_discussion(t.discussion).unwrap();
    let stored = view.posts.iter().find(|p| p.id == post.id).unwrap();
    assert_eq!(stored.context_ids, vec![t.right]);
    assert!(
        view.posts
            .iter()
            .filter(|p| p.id != post.id)
            .all(|p| p.context_ids.is_empty())
    );
}

#[test]
fn context_is_checked_and_tidied() {
    let mut t = thread();
    // The post being replied to is already there; duplicates count once.
    let post = t
        .repo
        .add_post_with_context(
            t.discussion,
            Some(t.left),
            t.me,
            "Tidy",
            &[t.left, t.right, t.right, t.question],
            at(3),
        )
        .unwrap();
    assert_eq!(post.context_ids, vec![t.right, t.question]);

    let (other, other_root) = t
        .repo
        .start_discussion("", t.me, "Elsewhere", at(4))
        .unwrap();
    let cross = t.repo.add_post_with_context(
        t.discussion,
        Some(t.left),
        t.me,
        "x",
        &[other_root.id],
        at(5),
    );
    assert!(matches!(cross, Err(Error::Invalid(m)) if m.contains("same discussion")));
    let missing = t.repo.add_post_with_context(
        t.discussion,
        Some(t.left),
        t.me,
        "x",
        &[Uuid::now_v7()],
        at(5),
    );
    assert!(matches!(missing, Err(Error::NotFound(_))));

    t.repo.delete_post(t.right, t.me, UnixMillis(0)).unwrap();
    let deleted =
        t.repo
            .add_post_with_context(t.discussion, Some(t.left), t.me, "x", &[t.right], at(6));
    assert!(matches!(deleted, Err(Error::Invalid(_))));
    let _ = other;
}

#[test]
fn drafts_remember_their_context_and_drop_deleted_posts() {
    let mut t = thread();
    t.repo
        .save_draft_with_context(
            t.discussion,
            Some(t.left),
            "",
            &[t.right, t.question],
            UnixMillis(1),
        )
        .unwrap();
    let draft = t.repo.get_discussion(t.discussion).unwrap().draft.unwrap();
    assert_eq!(draft.context_ids, vec![t.right, t.question]);

    t.repo.delete_post(t.right, t.me, UnixMillis(2)).unwrap();
    t.repo
        .save_draft_with_context(
            t.discussion,
            Some(t.left),
            "",
            &[t.right, t.question],
            UnixMillis(3),
        )
        .unwrap();
    let draft = t.repo.get_discussion(t.discussion).unwrap().draft.unwrap();
    assert_eq!(draft.context_ids, vec![t.question]);

    // Context alone keeps a draft; nothing at all removes it.
    t.repo
        .save_draft_with_context(t.discussion, None, "", &[t.question], UnixMillis(4))
        .unwrap();
    assert!(t.repo.get_discussion(t.discussion).unwrap().draft.is_some());
    t.repo
        .save_draft_with_context(t.discussion, None, "", &[], UnixMillis(5))
        .unwrap();
    assert!(t.repo.get_discussion(t.discussion).unwrap().draft.is_none());
}

#[test]
fn context_references_are_exported() {
    let mut t = thread();
    t.repo
        .add_post_with_context(t.discussion, Some(t.left), t.me, "Both", &[t.right], at(3))
        .unwrap();
    let view = t.repo.get_discussion(t.discussion).unwrap();
    let revisions = t.repo.discussion_revisions(t.discussion).unwrap();
    let json = nimata_core::archive::DiscussionArchive::new(&view, &revisions, t.me, UnixMillis(0))
        .to_json();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["posts"][3]["contextIds"][0], t.right.to_string());
    assert!(value["posts"][0].get("contextIds").is_none());
    assert!(nimata_core::archive::DiscussionArchive::from_json(&json).is_ok());
    assert_eq!(
        t.repo.list_discussions(DiscussionFilter::Active).unwrap()[0].post_count,
        4
    );
}
