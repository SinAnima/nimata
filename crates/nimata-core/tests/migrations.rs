//! Upgrading real databases written by earlier versions of Nimata.

use nimata_core::sqlite::{APPLICATION_ID, SCHEMA_VERSION};
use nimata_core::{DiscussionFilter, Repository, SqliteRepository, UnixMillis};
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use uuid::Uuid;

/// Loads a frozen SQL dump into a new database file.
fn database_from(fixture: &str, dir: &TempDir) -> PathBuf {
    let path = dir.path().join("nimata.sqlite3");
    let sql = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(fixture)).unwrap();
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute_batch(&sql)
        .unwrap();
    path
}

fn id(text: &str) -> Uuid {
    Uuid::parse_str(text).unwrap()
}

#[test]
fn a_version_1_database_upgrades_without_losing_anything() {
    let dir = TempDir::new().unwrap();
    let path = database_from("tests/fixtures/schema_v1.sql", &dir);

    let mut repo = SqliteRepository::open(&path).unwrap();

    let me = repo.local_user().unwrap();
    assert_eq!(me.id, id("01a10969-fa3e-754f-84da-3849ad072024"));
    assert_eq!(me.display_name, "Thanos");

    let active = repo.list_discussions(DiscussionFilter::Active).unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].post_count, 3);
    assert_eq!(
        repo.list_discussions(DiscussionFilter::Archived)
            .unwrap()
            .len(),
        1
    );

    let view = repo.get_discussion(active[0].id).unwrap();
    let posts: Vec<_> = view
        .posts
        .iter()
        .map(|p| {
            (
                p.body.as_str(),
                p.created_at.0,
                p.tz_offset_minutes,
                p.deleted_at,
            )
        })
        .collect();
    assert_eq!(
        posts,
        vec![
            ("Should Nimata use CouchDB?", 1_791_132_067_123, -240, None),
            (
                "The attraction is replication.",
                1_791_132_187_123,
                180,
                None
            ),
            (
                "But mobile changes the constraints.",
                1_791_132_307_123,
                345,
                None
            ),
        ]
    );
    assert_eq!(view.posts[2].parent_id, Some(view.posts[1].id));
    let draft = view.draft.unwrap();
    assert_eq!(draft.body, "Half a thought");
    assert_eq!(draft.parent_id, Some(view.posts[2].id));
}

#[test]
fn an_upgraded_database_supports_the_new_features() {
    let dir = TempDir::new().unwrap();
    let path = database_from("tests/fixtures/schema_v1.sql", &dir);
    let mut repo = SqliteRepository::open(&path).unwrap();
    let me = repo.local_user().unwrap();
    let post = id("01a10971-0000-7000-8000-000000000002");

    repo.edit_post(
        post,
        me.id,
        "Replication, mostly.",
        UnixMillis(1_791_200_000_000),
    )
    .unwrap();
    let revisions = repo.post_revisions(post).unwrap();
    assert_eq!(revisions[0].body, "The attraction is replication.");
    assert_eq!(revisions[0].written_at, UnixMillis(1_791_132_187_123));
    repo.delete_post(
        id("01a10971-0000-7000-8000-000000000003"),
        me.id,
        UnixMillis(1),
    )
    .unwrap();
    drop(repo);

    let conn = rusqlite::Connection::open(&path).unwrap();
    let (version, app_id): (i64, i64) = conn
        .query_row(
            "SELECT user_version, application_id FROM pragma_user_version, pragma_application_id",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((version, app_id), (SCHEMA_VERSION, APPLICATION_ID));
    let modified: i64 = conn
        .query_row(
            "SELECT modified_at FROM posts WHERE id = '01a10971-0000-7000-8000-000000000001'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        modified, 1_791_132_067_123,
        "existing rows get modified_at from their history"
    );
}

#[test]
fn reopening_an_upgraded_database_does_not_migrate_twice() {
    let dir = TempDir::new().unwrap();
    let path = database_from("tests/fixtures/schema_v1.sql", &dir);
    drop(SqliteRepository::open(&path).unwrap());
    let mut repo = SqliteRepository::open(&path).unwrap();
    assert_eq!(
        repo.list_discussions(DiscussionFilter::Active)
            .unwrap()
            .len(),
        1
    );
}
