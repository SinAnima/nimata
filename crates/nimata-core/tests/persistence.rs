//! Durability across reopening, and schema version handling.

use nimata_core::sqlite::SCHEMA_VERSION;
use nimata_core::{DiscussionFilter, Error, Repository, SqliteRepository, Timestamp, UnixMillis};
use tempfile::TempDir;

#[test]
fn everything_survives_closing_and_reopening() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nimata.sqlite3");
    let when = |m: i64| Timestamp {
        at: UnixMillis(1_791_132_067_123 + m * 60_000),
        offset_minutes: 180,
    };

    let (discussion_id, before) = {
        let mut repo = SqliteRepository::open(&path).unwrap();
        let me = repo.local_user().unwrap();
        let (d, q) = repo
            .start_discussion("", me.id, "Should Nimata use CouchDB?", when(0))
            .unwrap();
        let a = repo
            .add_post(
                d.id,
                Some(q.id),
                me.id,
                "The attraction is replication.",
                when(1),
            )
            .unwrap();
        let b = repo
            .add_post(
                d.id,
                Some(a.id),
                me.id,
                "But mobile changes the constraints.",
                when(2),
            )
            .unwrap();
        repo.save_draft(d.id, Some(b.id), "Half a thought", UnixMillis(1))
            .unwrap();
        (d.id, repo.get_discussion(d.id).unwrap())
    };

    let mut repo = SqliteRepository::open(&path).unwrap();
    assert_eq!(repo.get_discussion(discussion_id).unwrap(), before);
    assert_eq!(
        repo.list_discussions(DiscussionFilter::Active)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(repo.local_user().unwrap().id, before.participants[0].id);
}

#[test]
fn a_new_database_is_at_the_current_schema_version() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nimata.sqlite3");
    SqliteRepository::open(&path).unwrap();

    let conn = rusqlite::Connection::open(&path).unwrap();
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);
}

#[test]
fn a_database_from_a_newer_version_is_refused_untouched() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nimata.sqlite3");
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .unwrap();
    }

    let result = SqliteRepository::open(&path);
    assert!(matches!(result, Err(Error::NewerSchema { .. })));

    let conn = rusqlite::Connection::open(&path).unwrap();
    let tables: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(tables, 0, "no migration ran");
}
