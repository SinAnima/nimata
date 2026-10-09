//! Export everything, start again on a fresh device, import, and compare:
//! relationships, timestamps, history, and authorship must survive.

use nimata_core::archive::DiscussionArchive;
use nimata_core::domain::{GenerationStatus, ProviderKind, ProviderMetadata};
use nimata_core::import::{
    ImportFile, ImportResult, ImportSource, export_all, read_import, write_archive,
};
use nimata_core::repository::GenerationOutcome;
use nimata_core::search::parse;
use nimata_core::{Repository, SqliteRepository, Timestamp, UnixMillis};
use serde_json::Value;
use uuid::Uuid;

/// 2026-09-20 12:00 UTC.
const T0: i64 = 1_789_905_600_000;
const MIN: i64 = 60_000;

fn at(minutes: i64, offset: i32) -> Timestamp {
    Timestamp {
        at: UnixMillis(T0 + minutes * MIN),
        offset_minutes: offset,
    }
}

struct Device {
    repo: SqliteRepository,
    me: Uuid,
}

fn device() -> Device {
    let mut repo = SqliteRepository::open_in_memory().unwrap();
    let me = repo.local_user().unwrap().id;
    Device { repo, me }
}

/// Discussions with everything an archive has to carry: replies across
/// time zones, a model answer with metadata, an edit with its history, a
/// deleted post, a context reference, and an archived discussion.
fn notebook() -> Device {
    let mut d = device();
    d.repo.rename_participant(d.me, "Thanos").unwrap();
    let provider = d
        .repo
        .standard_provider(ProviderKind::Anthropic, UnixMillis(0))
        .unwrap();
    let claude = d
        .repo
        .set_model(provider.id, "claude-opus-5-5", "Claude Opus", true)
        .unwrap()
        .participant
        .id;

    let (couch, q) = d
        .repo
        .start_discussion(
            "Should Nimata use CouchDB?",
            d.me,
            "Should Nimata use CouchDB?",
            at(0, -240),
        )
        .unwrap();
    let (answer, generation) = d
        .repo
        .begin_generation(couch.id, q.id, claude, &[q.id], at(1, 0))
        .unwrap();
    d.repo
        .finish_generation(
            generation.id,
            GenerationOutcome {
                status: GenerationStatus::Complete,
                body: "Probably not; SQLite is enough.".into(),
                metadata: Some(ProviderMetadata {
                    provider: "anthropic".into(),
                    model: Some("claude-opus-5-5-20260901".into()),
                    response_id: Some("msg_01".into()),
                    request_id: Some("req_01".into()),
                    input_tokens: Some(120),
                    output_tokens: Some(9),
                    incomplete_reason: None,
                }),
                error: None,
            },
            UnixMillis(T0 + 2 * MIN),
        )
        .unwrap();
    let reply = d
        .repo
        .add_post(
            couch.id,
            Some(answer.id),
            d.me,
            "The attraction is replicaton.",
            at(3, 180),
        )
        .unwrap();
    d.repo
        .edit_post(
            reply.id,
            d.me,
            "The attraction is replication.",
            UnixMillis(T0 + 4 * MIN),
        )
        .unwrap();
    d.repo
        .add_post_with_context(
            couch.id,
            Some(q.id),
            d.me,
            "Compare with the answer above.",
            &[answer.id],
            at(5, 0),
        )
        .unwrap();
    let regret = d
        .repo
        .add_post(couch.id, Some(q.id), d.me, "Regrettable.", at(6, 0))
        .unwrap();
    d.repo
        .delete_post(regret.id, d.me, UnixMillis(T0 + 7 * MIN))
        .unwrap();

    let (old, _) = d
        .repo
        .start_discussion("Νήματα", d.me, "Threads, in Greek.", at(10, 120))
        .unwrap();
    d.repo
        .set_archived(old.id, true, UnixMillis(T0 + 11 * MIN))
        .unwrap();
    d
}

/// An archive as JSON with what legitimately differs between devices
/// removed: export time, and the local user's ID and name.
fn comparable(archive: &DiscussionArchive) -> Value {
    let mut v: Value = serde_json::to_value(archive).unwrap();
    v["exportedAt"] = Value::Null;
    let me = archive
        .participants
        .iter()
        .find(|p| p.local_user)
        .map(|p| p.id.to_string());
    for p in v["participants"].as_array_mut().unwrap() {
        if Some(p["id"].as_str().unwrap().to_string()) == me {
            p["id"] = "ME".into();
            p["displayName"] = "ME".into();
        }
    }
    for p in v["posts"].as_array_mut().unwrap() {
        if Some(p["authorId"].as_str().unwrap().to_string()) == me {
            p["authorId"] = "ME".into();
        }
    }
    v["participants"]
        .as_array_mut()
        .unwrap()
        .sort_by_key(|p| p["id"].to_string());
    v
}

fn zipped(archives: &[DiscussionArchive]) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    write_archive(&mut out, archives, UnixMillis(T0 + 60 * MIN), |_| {
        Err(nimata_core::Error::NotFound("file"))
    })
    .unwrap();
    out.into_inner()
}

fn import_all(d: &mut Device, bytes: &[u8]) -> Vec<nimata_core::import::ImportOutcome> {
    let ImportFile {
        source: _,
        discussions: archives,
        ..
    } = read_import(bytes).unwrap();
    archives
        .iter()
        .map(|a| {
            d.repo
                .import_discussion(a, UnixMillis(T0 + 90 * MIN), &|_| false)
                .unwrap()
        })
        .collect()
}

#[test]
fn an_archive_restores_everything_on_a_fresh_device() {
    let mut original = notebook();
    let exported = export_all(&mut original.repo, UnixMillis(0)).unwrap();
    assert_eq!(exported.len(), 2, "archived discussions are included");

    let mut fresh = device();
    let outcomes = import_all(&mut fresh, &zipped(&exported));
    assert!(outcomes.iter().all(|o| o.result == ImportResult::Added));
    assert_eq!(outcomes.iter().map(|o| o.posts_added).sum::<usize>(), 6);

    let again = export_all(&mut fresh.repo, UnixMillis(0)).unwrap();
    let key = |a: &DiscussionArchive| a.discussion.id;
    let (mut before, mut after) = (exported.clone(), again.clone());
    before.sort_by_key(key);
    after.sort_by_key(key);
    for (b, a) in before.iter().zip(&after) {
        assert_eq!(comparable(a), comparable(b));
    }

    // My posts are mine on the new device: I can edit them.
    let couch = after
        .iter()
        .find(|a| a.discussion.title.contains("CouchDB"))
        .unwrap();
    let mine = couch
        .posts
        .iter()
        .find(|p| p.body.starts_with("The attraction"))
        .unwrap();
    assert_eq!(mine.revisions.len(), 1);
    fresh
        .repo
        .edit_post(
            mine.id,
            fresh.me,
            "Replication, mostly.",
            UnixMillis(T0 + 100 * MIN),
        )
        .unwrap();
    // And imported posts are searchable.
    assert_eq!(
        fresh
            .repo
            .search(&parse("replication"), 10)
            .unwrap()
            .posts
            .len(),
        1
    );
    assert_eq!(
        fresh
            .repo
            .search(&parse("νηματα in:archived"), 10)
            .unwrap()
            .discussions
            .len(),
        1
    );
}

#[test]
fn importing_the_same_archive_again_changes_nothing() {
    let mut original = notebook();
    let bytes = zipped(&export_all(&mut original.repo, UnixMillis(0)).unwrap());
    let mut fresh = device();
    import_all(&mut fresh, &bytes);
    let first = export_all(&mut fresh.repo, UnixMillis(0)).unwrap();

    let outcomes = import_all(&mut fresh, &bytes);
    assert!(
        outcomes
            .iter()
            .all(|o| o.result == ImportResult::Unchanged && o.posts_added == 0)
    );
    assert_eq!(outcomes.iter().map(|o| o.posts_present).sum::<usize>(), 6);
    let second = export_all(&mut fresh.repo, UnixMillis(0)).unwrap();
    assert_eq!(second, first);

    // Importing on the device that exported it is a no-op too.
    let outcomes = import_all(&mut original, &bytes);
    assert!(outcomes.iter().all(|o| o.result == ImportResult::Unchanged));
}

#[test]
fn a_newer_export_adds_only_what_is_missing_and_keeps_local_changes() {
    let mut original = notebook();
    let older = zipped(&export_all(&mut original.repo, UnixMillis(0)).unwrap());
    let mut fresh = device();
    import_all(&mut fresh, &older);

    // Both devices change things after the first import.
    let couch = original
        .repo
        .list_discussions(nimata_core::DiscussionFilter::Active)
        .unwrap()[0]
        .id;
    let root = original.repo.get_discussion(couch).unwrap().posts[0].id;
    original
        .repo
        .add_post(
            couch,
            Some(root),
            original.me,
            "A later thought.",
            at(30, 0),
        )
        .unwrap();
    let local = fresh.repo.get_discussion(couch).unwrap().posts[0].clone();
    fresh
        .repo
        .rename_discussion(couch, "CouchDB (my copy)", UnixMillis(T0 + 31 * MIN))
        .unwrap();

    let outcomes = import_all(
        &mut fresh,
        &zipped(&export_all(&mut original.repo, UnixMillis(0)).unwrap()),
    );
    let couch_outcome = outcomes.iter().find(|o| o.discussion_id == couch).unwrap();
    assert_eq!(couch_outcome.result, ImportResult::Updated);
    assert_eq!(couch_outcome.posts_added, 1);

    let view = fresh.repo.get_discussion(couch).unwrap();
    assert_eq!(
        view.discussion.title, "CouchDB (my copy)",
        "local changes are kept"
    );
    assert_eq!(view.posts[0], local);
    assert_eq!(view.posts.last().unwrap().body, "A later thought.");
    assert_eq!(view.posts.last().unwrap().author_id, fresh.me);
    assert_eq!(view.discussion.updated_at, UnixMillis(T0 + 30 * MIN));
}

#[test]
fn a_discussion_deleted_here_stays_deleted() {
    let mut original = notebook();
    let bytes = zipped(&export_all(&mut original.repo, UnixMillis(0)).unwrap());
    let couch = original
        .repo
        .list_discussions(nimata_core::DiscussionFilter::Active)
        .unwrap()[0]
        .id;
    original
        .repo
        .delete_discussion(couch, UnixMillis(T0 + 50 * MIN))
        .unwrap();

    let outcomes = import_all(&mut original, &bytes);
    let o = outcomes.iter().find(|o| o.discussion_id == couch).unwrap();
    assert_eq!(o.result, ImportResult::SkippedDeleted);
    assert!(original.repo.get_discussion(couch).is_err());
}

#[test]
fn a_conflicting_archive_changes_nothing() {
    let mut original = notebook();
    let mut archives = export_all(&mut original.repo, UnixMillis(0)).unwrap();
    // A discussion claiming a post that already belongs to another one.
    let stolen = archives[0].posts[0].clone();
    let mut forged = archives.remove(1);
    forged.discussion.id = Uuid::now_v7();
    forged.posts.push(nimata_core::archive::ArchivedPost {
        parent_id: None,
        ..stolen
    });
    let error = original
        .repo
        .import_discussion(&forged, UnixMillis(0), &|_| false)
        .unwrap_err();
    assert!(error.to_string().contains("another discussion"), "{error}");
    assert!(
        original.repo.get_discussion(forged.discussion.id).is_err(),
        "rolled back"
    );
}

#[test]
fn a_single_discussion_file_imports_too() {
    let mut original = notebook();
    let one = export_all(&mut original.repo, UnixMillis(0))
        .unwrap()
        .remove(0);
    let ImportFile {
        source,
        discussions: archives,
        ..
    } = read_import(one.to_json().as_bytes()).unwrap();
    assert_eq!(source, ImportSource::NimataDiscussion);
    let mut fresh = device();
    let o = fresh
        .repo
        .import_discussion(&archives[0], UnixMillis(0), &|_| false)
        .unwrap();
    assert_eq!((o.result, o.posts_added), (ImportResult::Added, 5));
}

#[test]
fn chatgpt_conversations_become_discussions_once() {
    let export = serde_json::json!([{
        "title": "Datalog", "create_time": 1_791_000_000.0, "conversation_id": "conv-1",
        "mapping": {
            "u": { "parent": null, "message": {
                "id": "m-u", "author": { "role": "user" }, "create_time": 1_791_000_010.0, "recipient": "all",
                "content": { "content_type": "text", "parts": ["Could Datalog replace the rules?"] }, "metadata": {} } },
            "a": { "parent": "u", "message": {
                "id": "m-a", "author": { "role": "assistant" }, "create_time": 1_791_000_020.0, "recipient": "all",
                "content": { "content_type": "text", "parts": ["With stratification."] },
                "metadata": { "model_slug": "gpt-4o" } } }
        }
    }])
    .to_string();
    let mut d = device();
    let ImportFile {
        source,
        discussions: archives,
        ..
    } = read_import(export.as_bytes()).unwrap();
    assert_eq!(source, ImportSource::ChatGpt);
    let o = d
        .repo
        .import_discussion(&archives[0], UnixMillis(0), &|_| false)
        .unwrap();
    assert_eq!((o.result, o.posts_added), (ImportResult::Added, 2));

    let view = d.repo.get_discussion(o.discussion_id).unwrap();
    assert_eq!(view.posts[0].author_id, d.me, "your messages are yours");
    assert_eq!(view.posts[1].parent_id, Some(view.posts[0].id));
    let gpt = view
        .participants
        .iter()
        .find(|p| p.id == view.posts[1].author_id)
        .unwrap();
    assert_eq!(gpt.display_name, "ChatGPT (gpt-4o)");
    assert_eq!(
        view.posts[1]
            .provider_metadata
            .as_ref()
            .unwrap()
            .response_id
            .as_deref(),
        Some("m-a")
    );
    // Imported models are authors, not models that can be asked.
    assert!(d.repo.model_participants().unwrap().is_empty());

    let ImportFile {
        source: _,
        discussions: again,
        ..
    } = read_import(export.as_bytes()).unwrap();
    let o = d
        .repo
        .import_discussion(&again[0], UnixMillis(0), &|_| false)
        .unwrap();
    assert_eq!(o.result, ImportResult::Unchanged);
}
