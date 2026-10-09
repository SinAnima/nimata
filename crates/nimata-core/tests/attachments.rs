//! Attachments from file to post to archive and backup, and back.

use nimata_core::attachments::{AttachmentKind, BlobStore, StagedAttachment, classify};
use nimata_core::import::{ImportFile, ImportResult, export_all, read_import, write_archive};
use nimata_core::{Repository, SqliteRepository, Timestamp, UnixMillis};
use tempfile::TempDir;
use uuid::Uuid;

fn at(minutes: i64) -> Timestamp {
    Timestamp {
        at: UnixMillis(1_789_905_600_000 + minutes * 60_000),
        offset_minutes: 120,
    }
}

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n not really an image, but it starts like one";

struct Device {
    repo: SqliteRepository,
    store: BlobStore,
    me: Uuid,
    _dir: TempDir,
}

fn device() -> Device {
    let dir = TempDir::new().unwrap();
    let mut repo = SqliteRepository::open(&dir.path().join("nimata.sqlite3")).unwrap();
    let me = repo.local_user().unwrap().id;
    Device {
        repo,
        store: BlobStore::new(dir.path().join("blobs")),
        me,
        _dir: dir,
    }
}

/// What the app does when a file is added: store the bytes, then describe
/// them.
fn stage(d: &Device, name: &str, reported: &str, bytes: &[u8]) -> StagedAttachment {
    let (media_type, kind) = classify(name, reported, bytes);
    StagedAttachment {
        filename: name.into(),
        media_type,
        size: bytes.len() as u64,
        content_hash: d.store.put(bytes).unwrap(),
        kind,
    }
}

#[test]
fn files_are_attached_kept_with_drafts_and_deleted_with_their_post() {
    let mut d = device();
    let notes = stage(&d, "notes.md", "", b"# Notes\n\nStratification matters.");
    let image = stage(&d, "diagram.png", "image/png", PNG);
    assert_eq!(notes.kind, AttachmentKind::Text);
    assert_eq!(image.kind, AttachmentKind::Image);

    // A post may be just a file; the discussion is then named after it.
    let (discussion, first) = d
        .repo
        .start_discussion_with_attachments("", d.me, "", std::slice::from_ref(&notes), at(0))
        .unwrap();
    assert_eq!(discussion.title, "notes.md");
    assert_eq!(first.body, "");
    assert!(
        d.repo.start_discussion("", d.me, "  ", at(0)).is_err(),
        "nothing to post"
    );

    // Files wait in the draft, across restarts.
    d.repo
        .save_draft_with_attachments(
            discussion.id,
            Some(first.id),
            "",
            &[],
            std::slice::from_ref(&image),
            UnixMillis(1),
        )
        .unwrap();
    let draft = d.repo.get_discussion(discussion.id).unwrap().draft.unwrap();
    assert_eq!(draft.attachments, vec![image.clone()]);
    assert!(d.repo.hash_in_use(&image.content_hash).unwrap());
    assert_eq!(
        d.repo.image_type(&image.content_hash).unwrap().as_deref(),
        Some("image/png")
    );
    assert_eq!(
        d.repo.image_type(&notes.content_hash).unwrap(),
        None,
        "only images are shown"
    );

    // Posting moves them from the draft to the post. The same file attached
    // twice is stored once.
    let reply = d
        .repo
        .add_post_with_attachments(
            discussion.id,
            Some(first.id),
            d.me,
            "The diagram, and the notes again.",
            &[],
            &[image.clone(), notes.clone()],
            at(1),
        )
        .unwrap();
    let view = d.repo.get_discussion(discussion.id).unwrap();
    assert!(view.draft.is_none());
    let names: Vec<(&str, Uuid)> = view
        .attachments
        .iter()
        .map(|a| (a.filename.as_str(), a.post_id))
        .collect();
    assert_eq!(
        names,
        vec![
            ("notes.md", first.id),
            ("diagram.png", reply.id),
            ("notes.md", reply.id)
        ]
    );
    assert_eq!(
        view.attachments[0].content_hash,
        view.attachments[2].content_hash
    );
    let mut hashes = d.repo.hashes_in_use().unwrap();
    hashes.sort();
    let mut expected = vec![notes.content_hash.clone(), image.content_hash.clone()];
    expected.sort();
    assert_eq!(hashes, expected);

    // Deleting the reply removes its attachments; the notes are still used
    // by the first post, the image by nothing.
    d.repo.delete_post(reply.id, d.me, UnixMillis(2)).unwrap();
    assert!(d.repo.post_attachments(&[reply.id]).unwrap().is_empty());
    assert!(d.repo.hash_in_use(&notes.content_hash).unwrap());
    assert!(!d.repo.hash_in_use(&image.content_hash).unwrap());

    d.repo
        .delete_discussion(discussion.id, UnixMillis(3))
        .unwrap();
    assert!(d.repo.hashes_in_use().unwrap().is_empty());
}

#[test]
fn staged_files_are_checked() {
    let mut d = device();
    let (discussion, first) = d.repo.start_discussion("T", d.me, "Q", at(0)).unwrap();
    let mut bad = stage(&d, "a.md", "", b"x");
    bad.content_hash = "sha256:../../etc/passwd".into();
    assert!(
        d.repo
            .add_post_with_attachments(discussion.id, Some(first.id), d.me, "", &[], &[bad], at(1))
            .is_err()
    );
    let mut sneaky = stage(&d, "a.md", "", b"x");
    sneaky.filename = "../../.ssh/config".into();
    d.repo
        .add_post_with_attachments(
            discussion.id,
            Some(first.id),
            d.me,
            "",
            &[],
            &[sneaky],
            at(1),
        )
        .unwrap();
    let view = d.repo.get_discussion(discussion.id).unwrap();
    assert_eq!(view.attachments[0].filename, "config");
}

fn notebook_with_files() -> (Device, Vec<StagedAttachment>) {
    let mut d = device();
    let files = vec![
        stage(&d, "notes.md", "text/markdown", b"# Notes"),
        stage(&d, "diagram.png", "", PNG),
    ];
    d.repo
        .start_discussion_with_attachments("Datalog", d.me, "Have a look.", &files, at(0))
        .unwrap();
    (d, files)
}

fn archive_of(d: &mut Device) -> Vec<u8> {
    let archives = export_all(&mut d.repo, UnixMillis(0)).unwrap();
    let mut out = std::io::Cursor::new(Vec::new());
    write_archive(&mut out, &archives, UnixMillis(0), |hash| {
        d.store.read(hash)
    })
    .unwrap();
    out.into_inner()
}

fn import(d: &mut Device, bytes: &[u8]) -> Vec<nimata_core::import::ImportOutcome> {
    let ImportFile {
        discussions, files, ..
    } = read_import(bytes).unwrap();
    for bytes in files.values() {
        d.store.put(bytes).unwrap();
    }
    let store = d.store.clone();
    discussions
        .iter()
        .map(|a| {
            d.repo
                .import_discussion(a, UnixMillis(5), &|hash| store.contains(hash))
                .unwrap()
        })
        .collect()
}

#[test]
fn archives_carry_the_files_and_imports_restore_them() {
    let (mut original, files) = notebook_with_files();
    let bytes = archive_of(&mut original);

    let mut fresh = device();
    let outcomes = import(&mut fresh, &bytes);
    assert_eq!(outcomes[0].result, ImportResult::Added);
    assert_eq!(
        (
            outcomes[0].attachments_added,
            outcomes[0].attachments_missing
        ),
        (2, 0)
    );
    let id = outcomes[0].discussion_id;
    let view = fresh.repo.get_discussion(id).unwrap();
    let original_view = original.repo.get_discussion(id).unwrap();
    assert_eq!(
        view.attachments, original_view.attachments,
        "same IDs, names, types, and times"
    );
    for f in &files {
        assert_eq!(
            fresh.store.read(&f.content_hash).unwrap(),
            original.store.read(&f.content_hash).unwrap()
        );
    }

    // Again: nothing new.
    let again = import(&mut fresh, &bytes);
    assert_eq!(
        (again[0].result, again[0].attachments_added),
        (ImportResult::Unchanged, 0)
    );
}

#[test]
fn files_missing_from_an_archive_are_counted_not_invented() {
    let (mut original, files) = notebook_with_files();
    original.store.remove(&files[1].content_hash).unwrap();
    let bytes = archive_of(&mut original);

    let mut fresh = device();
    let outcomes = import(&mut fresh, &bytes);
    assert_eq!(
        (
            outcomes[0].attachments_added,
            outcomes[0].attachments_missing
        ),
        (1, 1)
    );
    let view = fresh
        .repo
        .get_discussion(outcomes[0].discussion_id)
        .unwrap();
    assert_eq!(view.attachments.len(), 1);
    assert_eq!(view.attachments[0].filename, "notes.md");
}

#[test]
fn a_backup_holds_the_files_too() {
    let (original, files) = notebook_with_files();
    let dir = TempDir::new().unwrap();
    let backup = dir.path().join("backup.sqlite3");
    original
        .repo
        .backup_with_files(&backup, Some(&original.store))
        .unwrap();
    assert_eq!(
        std::fs::read_dir(dir.path()).unwrap().count(),
        1,
        "one self-contained file"
    );

    let mut fresh = device();
    fresh
        .repo
        .restore_with_files(&backup, Some(&fresh.store))
        .unwrap();
    for f in &files {
        assert!(fresh.store.contains(&f.content_hash));
    }
    let discussion = fresh
        .repo
        .list_discussions(nimata_core::DiscussionFilter::Active)
        .unwrap()[0]
        .id;
    assert_eq!(
        fresh
            .repo
            .get_discussion(discussion)
            .unwrap()
            .attachments
            .len(),
        2
    );
    // The carried copies are not kept in the database.
    let again = dir.path().join("again.sqlite3");
    fresh.repo.backup_to(&again).unwrap();
    let conn = rusqlite::Connection::open(&again).unwrap();
    let tables: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE name = 'backup_files'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(tables, 0);
}
