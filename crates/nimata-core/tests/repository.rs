//! Repository contract tests. Every test runs against an in-memory database
//! and an on-disk database through the `Repository` trait.

use nimata_core::{DiscussionFilter, Error, Repository, SqliteRepository, Timestamp, UnixMillis};
use tempfile::TempDir;

/// 2026-10-04 16:41:07.123 UTC.
const T0: i64 = 1_791_132_067_123;
const MINUTE: i64 = 60_000;

fn at(minutes: i64, offset_minutes: i32) -> Timestamp {
    Timestamp {
        at: UnixMillis(T0 + minutes * MINUTE),
        offset_minutes,
    }
}

fn for_each_repo(test: impl Fn(&mut dyn Repository)) {
    test(&mut SqliteRepository::open_in_memory().unwrap());
    let dir = TempDir::new().unwrap();
    test(&mut SqliteRepository::open(&dir.path().join("nimata.sqlite3")).unwrap());
}

#[test]
fn local_user_is_created_once_and_can_be_renamed() {
    for_each_repo(|repo| {
        let me = repo.local_user().unwrap();
        assert_eq!(me.display_name, "Me");
        assert_eq!(repo.local_user().unwrap().id, me.id);

        let renamed = repo.rename_participant(me.id, "  Thanos ").unwrap();
        assert_eq!(renamed.display_name, "Thanos");
        assert_eq!(repo.local_user().unwrap().display_name, "Thanos");
        assert!(matches!(
            repo.rename_participant(me.id, " "),
            Err(Error::Invalid(_))
        ));
    });
}

#[test]
fn starting_a_discussion_creates_its_first_post() {
    for_each_repo(|repo| {
        let me = repo.local_user().unwrap();
        let (d, root) = repo
            .start_discussion("CouchDB?", me.id, "Should Nimata use CouchDB?", at(0, -240))
            .unwrap();
        assert_eq!(root.parent_id, None);

        let view = repo.get_discussion(d.id).unwrap();
        assert_eq!(view.discussion.title, "CouchDB?");
        assert_eq!(view.posts, vec![root]);
        assert_eq!(view.participants, vec![me]);
    });
}

#[test]
fn an_empty_title_comes_from_the_first_line_of_the_post() {
    for_each_repo(|repo| {
        let me = repo.local_user().unwrap();
        let (d, _) = repo
            .start_discussion("  ", me.id, "\nShould Nimata use CouchDB?\nMore.", at(0, 0))
            .unwrap();
        assert_eq!(d.title, "Should Nimata use CouchDB?");
    });
}

#[test]
fn empty_posts_are_rejected_without_creating_anything() {
    for_each_repo(|repo| {
        let me = repo.local_user().unwrap();
        let result = repo.start_discussion("Title", me.id, " \n ", at(0, 0));
        assert!(matches!(result, Err(Error::Invalid(_))));
        assert!(
            repo.list_discussions(DiscussionFilter::Active)
                .unwrap()
                .is_empty()
        );
    });
}

#[test]
fn replies_can_target_any_earlier_post() {
    for_each_repo(|repo| {
        let me = repo.local_user().unwrap();
        let (d, root) = repo
            .start_discussion("", me.id, "Should Nimata use CouchDB?", at(0, 0))
            .unwrap();
        let a = repo
            .add_post(
                d.id,
                Some(root.id),
                me.id,
                "The attraction is replication.",
                at(1, 0),
            )
            .unwrap();
        let b = repo
            .add_post(
                d.id,
                Some(a.id),
                me.id,
                "But mobile changes the constraints.",
                at(2, 0),
            )
            .unwrap();
        let c = repo
            .add_post(
                d.id,
                Some(root.id),
                me.id,
                "A second branch off the root.",
                at(3, 0),
            )
            .unwrap();

        let view = repo.get_discussion(d.id).unwrap();
        let ids: Vec<_> = view.posts.iter().map(|p| p.id).collect();
        assert_eq!(ids, vec![root.id, a.id, b.id, c.id], "chronological order");
        assert_eq!(view.posts[3].parent_id, Some(root.id));
    });
}

#[test]
fn a_reply_must_stay_in_its_discussion() {
    for_each_repo(|repo| {
        let me = repo.local_user().unwrap();
        let (d1, p1) = repo
            .start_discussion("One", me.id, "first", at(0, 0))
            .unwrap();
        let (d2, _) = repo
            .start_discussion("Two", me.id, "second", at(1, 0))
            .unwrap();

        let cross = repo.add_post(d2.id, Some(p1.id), me.id, "reply", at(2, 0));
        assert!(matches!(cross, Err(Error::Invalid(_))));

        let missing = repo.add_post(d1.id, Some(uuid::Uuid::now_v7()), me.id, "reply", at(2, 0));
        assert!(matches!(missing, Err(Error::NotFound(_))));

        let no_discussion = repo.add_post(uuid::Uuid::now_v7(), None, me.id, "x", at(2, 0));
        assert!(matches!(no_discussion, Err(Error::NotFound(_))));

        assert_eq!(repo.get_discussion(d2.id).unwrap().posts.len(), 1);
    });
}

#[test]
fn timestamps_and_offsets_round_trip_exactly() {
    for_each_repo(|repo| {
        let me = repo.local_user().unwrap();
        let (d, root) = repo
            .start_discussion("T", me.id, "Athens", at(0, 180))
            .unwrap();
        repo.add_post(d.id, Some(root.id), me.id, "Kathmandu", at(1, 345))
            .unwrap();
        repo.add_post(d.id, Some(root.id), me.id, "Honolulu", at(2, -600))
            .unwrap();

        let posts = repo.get_discussion(d.id).unwrap().posts;
        let stored: Vec<_> = posts
            .iter()
            .map(|p| (p.created_at.0, p.tz_offset_minutes))
            .collect();
        assert_eq!(
            stored,
            vec![(T0, 180), (T0 + MINUTE, 345), (T0 + 2 * MINUTE, -600)]
        );
    });
}

#[test]
fn discussions_are_listed_by_latest_activity() {
    for_each_repo(|repo| {
        let me = repo.local_user().unwrap();
        let (old, old_root) = repo
            .start_discussion("Old", me.id, "old", at(0, 0))
            .unwrap();
        let (new, _) = repo
            .start_discussion("New", me.id, "new", at(10, 0))
            .unwrap();

        let titles = |repo: &mut dyn Repository| {
            repo.list_discussions(DiscussionFilter::Active)
                .unwrap()
                .into_iter()
                .map(|s| s.title)
                .collect::<Vec<_>>()
        };
        assert_eq!(titles(repo), vec!["New", "Old"]);

        repo.add_post(old.id, Some(old_root.id), me.id, "latest words", at(20, 0))
            .unwrap();
        assert_eq!(titles(repo), vec!["Old", "New"]);

        let summary = &repo.list_discussions(DiscussionFilter::Active).unwrap()[0];
        assert_eq!(summary.post_count, 2);
        assert_eq!(summary.excerpt, "latest words");
        assert_eq!(summary.last_activity_at, UnixMillis(T0 + 20 * MINUTE));

        // Renaming is not activity.
        repo.rename_discussion(new.id, "Renamed", UnixMillis(T0))
            .unwrap();
        assert_eq!(titles(repo), vec!["Old", "Renamed"]);
    });
}

#[test]
fn renaming_trims_and_rejects_empty_titles() {
    for_each_repo(|repo| {
        let me = repo.local_user().unwrap();
        let (d, _) = repo
            .start_discussion("Before", me.id, "x", at(0, 0))
            .unwrap();
        assert_eq!(
            repo.rename_discussion(d.id, "  After ", UnixMillis(T0))
                .unwrap()
                .title,
            "After"
        );
        assert!(matches!(
            repo.rename_discussion(d.id, "", UnixMillis(T0)),
            Err(Error::Invalid(_))
        ));
        assert!(matches!(
            repo.rename_discussion(uuid::Uuid::now_v7(), "x", UnixMillis(T0)),
            Err(Error::NotFound(_))
        ));
    });
}

#[test]
fn archiving_moves_a_discussion_between_lists() {
    for_each_repo(|repo| {
        let me = repo.local_user().unwrap();
        let (d, _) = repo.start_discussion("Keep", me.id, "x", at(0, 0)).unwrap();

        let archived = repo
            .set_archived(d.id, true, UnixMillis(T0 + MINUTE))
            .unwrap();
        assert_eq!(archived.archived_at, Some(UnixMillis(T0 + MINUTE)));
        assert!(
            repo.list_discussions(DiscussionFilter::Active)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            repo.list_discussions(DiscussionFilter::Archived)
                .unwrap()
                .len(),
            1
        );

        let restored = repo
            .set_archived(d.id, false, UnixMillis(T0 + 2 * MINUTE))
            .unwrap();
        assert_eq!(restored.archived_at, None);
        assert_eq!(
            repo.list_discussions(DiscussionFilter::Active)
                .unwrap()
                .len(),
            1
        );
    });
}

#[test]
fn drafts_are_saved_replaced_and_cleared_by_posting() {
    for_each_repo(|repo| {
        let me = repo.local_user().unwrap();
        let (d, root) = repo.start_discussion("D", me.id, "root", at(0, 0)).unwrap();

        repo.save_draft(d.id, None, "first try", UnixMillis(T0 + 1))
            .unwrap();
        repo.save_draft(d.id, Some(root.id), "second try", UnixMillis(T0 + 2))
            .unwrap();
        let draft = repo.get_discussion(d.id).unwrap().draft.unwrap();
        assert_eq!(draft.body, "second try");
        assert_eq!(draft.parent_id, Some(root.id));

        repo.add_post(d.id, Some(root.id), me.id, "second try", at(1, 0))
            .unwrap();
        assert_eq!(repo.get_discussion(d.id).unwrap().draft, None);
    });
}

#[test]
fn an_empty_draft_without_a_reply_target_is_removed() {
    for_each_repo(|repo| {
        let me = repo.local_user().unwrap();
        let (d, root) = repo.start_discussion("D", me.id, "root", at(0, 0)).unwrap();

        repo.save_draft(d.id, None, "words", UnixMillis(T0))
            .unwrap();
        assert_eq!(
            repo.save_draft(d.id, None, "  ", UnixMillis(T0)).unwrap(),
            None
        );
        assert_eq!(repo.get_discussion(d.id).unwrap().draft, None);

        // A chosen reply target is worth keeping even before any text.
        assert!(
            repo.save_draft(d.id, Some(root.id), "", UnixMillis(T0))
                .unwrap()
                .is_some()
        );
    });
}

#[test]
fn editing_keeps_every_earlier_version() {
    for_each_repo(|repo| {
        let me = repo.local_user().unwrap();
        let (d, root) = repo
            .start_discussion("D", me.id, "first", at(0, 0))
            .unwrap();

        let edited = repo
            .edit_post(root.id, me.id, "second", UnixMillis(T0 + MINUTE))
            .unwrap();
        assert_eq!(edited.body, "second");
        assert_eq!(edited.edited_at, Some(UnixMillis(T0 + MINUTE)));
        assert_eq!(
            edited.created_at, root.created_at,
            "creation time is untouched"
        );
        repo.edit_post(root.id, me.id, "third", UnixMillis(T0 + 2 * MINUTE))
            .unwrap();

        let revisions = repo.post_revisions(root.id).unwrap();
        let history: Vec<_> = revisions
            .iter()
            .map(|r| (r.body.as_str(), r.written_at.0, r.replaced_at.0))
            .collect();
        assert_eq!(
            history,
            vec![
                ("first", T0, T0 + MINUTE),
                ("second", T0 + MINUTE, T0 + 2 * MINUTE)
            ]
        );
        assert_eq!(
            repo.discussion_revisions(d.id).unwrap()[&root.id],
            revisions
        );
        assert_eq!(repo.get_discussion(d.id).unwrap().posts[0].body, "third");
    });
}

#[test]
fn an_unchanged_edit_records_nothing() {
    for_each_repo(|repo| {
        let me = repo.local_user().unwrap();
        let (_, root) = repo.start_discussion("D", me.id, "same", at(0, 0)).unwrap();
        let after = repo
            .edit_post(root.id, me.id, "\nsame\n", UnixMillis(T0 + 1))
            .unwrap();
        assert_eq!(after.edited_at, None);
        assert!(repo.post_revisions(root.id).unwrap().is_empty());
    });
}

#[test]
fn only_the_author_can_edit_or_delete() {
    for_each_repo(|repo| {
        let me = repo.local_user().unwrap();
        let (_, root) = repo.start_discussion("D", me.id, "mine", at(0, 0)).unwrap();
        let someone = uuid::Uuid::now_v7();
        assert!(matches!(
            repo.edit_post(root.id, someone, "theirs", UnixMillis(T0)),
            Err(Error::Invalid(_))
        ));
        assert!(matches!(
            repo.delete_post(root.id, someone, UnixMillis(T0)),
            Err(Error::Invalid(_))
        ));
        assert!(matches!(
            repo.edit_post(uuid::Uuid::now_v7(), me.id, "x", UnixMillis(T0)),
            Err(Error::NotFound(_))
        ));
    });
}

#[test]
fn a_deleted_post_stays_in_the_thread_without_its_text() {
    for_each_repo(|repo| {
        let me = repo.local_user().unwrap();
        let (d, root) = repo
            .start_discussion("D", me.id, "question", at(0, 0))
            .unwrap();
        let reply = repo
            .add_post(d.id, Some(root.id), me.id, "regrettable", at(1, 0))
            .unwrap();
        let answer = repo
            .add_post(d.id, Some(reply.id), me.id, "answer", at(2, 0))
            .unwrap();
        repo.edit_post(
            reply.id,
            me.id,
            "more regrettable",
            UnixMillis(T0 + 3 * MINUTE),
        )
        .unwrap();
        repo.save_draft(d.id, Some(reply.id), "draft", UnixMillis(T0))
            .unwrap();

        let deleted = repo
            .delete_post(reply.id, me.id, UnixMillis(T0 + 4 * MINUTE))
            .unwrap();
        assert_eq!(deleted.body, "");
        assert_eq!(deleted.deleted_at, Some(UnixMillis(T0 + 4 * MINUTE)));

        let view = repo.get_discussion(d.id).unwrap();
        assert_eq!(view.posts.len(), 3, "the tombstone keeps its place");
        assert_eq!(
            view.posts[2].parent_id,
            Some(reply.id),
            "replies keep their parent"
        );
        assert!(
            repo.post_revisions(reply.id).unwrap().is_empty(),
            "history is erased too"
        );
        let draft = view.draft.unwrap();
        assert_eq!((draft.body.as_str(), draft.parent_id), ("draft", None));

        assert!(matches!(
            repo.add_post(d.id, Some(reply.id), me.id, "late", at(5, 0)),
            Err(Error::Invalid(_))
        ));
        assert!(matches!(
            repo.edit_post(reply.id, me.id, "back", UnixMillis(T0)),
            Err(Error::Invalid(_))
        ));

        let summary = &repo.list_discussions(DiscussionFilter::Active).unwrap()[0];
        assert_eq!(summary.post_count, 2);
        assert_eq!(summary.excerpt, "answer");
        let _ = answer;
    });
}

#[test]
fn a_deleted_discussion_disappears_everywhere() {
    for_each_repo(|repo| {
        let me = repo.local_user().unwrap();
        let (d, root) = repo
            .start_discussion("Private", me.id, "secret", at(0, 0))
            .unwrap();
        repo.edit_post(root.id, me.id, "more secret", UnixMillis(T0 + 1))
            .unwrap();
        repo.save_draft(d.id, None, "draft", UnixMillis(T0))
            .unwrap();

        repo.delete_discussion(d.id, UnixMillis(T0 + MINUTE))
            .unwrap();

        assert!(
            repo.list_discussions(DiscussionFilter::Active)
                .unwrap()
                .is_empty()
        );
        assert!(
            repo.list_discussions(DiscussionFilter::Archived)
                .unwrap()
                .is_empty()
        );
        assert!(matches!(repo.get_discussion(d.id), Err(Error::NotFound(_))));
        assert!(matches!(
            repo.add_post(d.id, None, me.id, "x", at(2, 0)),
            Err(Error::NotFound(_))
        ));
        assert!(matches!(
            repo.rename_discussion(d.id, "x", UnixMillis(T0)),
            Err(Error::NotFound(_))
        ));
        assert!(matches!(
            repo.delete_discussion(d.id, UnixMillis(T0)),
            Err(Error::NotFound(_))
        ));
    });
}
