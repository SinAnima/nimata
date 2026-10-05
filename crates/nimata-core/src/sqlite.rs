//! SQLite implementation of [`Repository`].
//!
//! IDs are stored as hyphenated UUID text and times as integers so the
//! database stays readable with ordinary SQLite tools.

use std::collections::HashMap;
use std::path::Path;

use rusqlite::{Connection, ErrorCode, OpenFlags, OptionalExtension, Row, params};
use uuid::Uuid;

use crate::domain::{
    Discussion, DiscussionFilter, DiscussionSummary, DiscussionView, Draft, Participant,
    ParticipantKind, Post, PostStatus, Revision, clean_body, clean_display_name, clean_title,
    excerpt, title_from_body,
};
use crate::error::{Error, Result};
use crate::repository::Repository;
use crate::time::{Timestamp, UnixMillis};

/// Schema migrations, applied in order. The schema version is the number
/// applied, recorded in `PRAGMA user_version`.
const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001_initial.sql"),
    include_str!("../migrations/0002_history_and_tombstones.sql"),
];

pub const SCHEMA_VERSION: i64 = MIGRATIONS.len() as i64;

/// `PRAGMA application_id` of a Nimata database: "NIMA" in ASCII. Set from
/// schema version 2; version 1 databases have 0.
pub const APPLICATION_ID: i64 = 0x4E49_4D41;

const LOCAL_USER_DEFAULT_NAME: &str = "Me";

pub struct SqliteRepository {
    conn: Connection,
}

impl SqliteRepository {
    /// Opens or creates the database at `path`, checking that an existing
    /// file is an undamaged Nimata database before migrating it.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        check_database(&conn, &path.display().to_string())?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        migrate(&mut conn)?;
        Ok(Self { conn })
    }

    /// Writes a complete, consistent copy of the database to `dest` using
    /// SQLite's online backup. The copy is written next to `dest` first and
    /// moved into place only after it has been verified, so an interrupted
    /// backup never leaves a partial file under the chosen name.
    pub fn backup_to(&self, dest: &Path) -> Result<()> {
        let partial = dest.with_extension("partial");
        let remove_partial = || {
            for suffix in ["", "-wal", "-shm"] {
                let _ = std::fs::remove_file(format!("{}{suffix}", partial.display()));
            }
        };
        remove_partial();
        let result = (|| {
            self.conn.backup(rusqlite::MAIN_DB, &partial, None)?;
            let copy = Connection::open(&partial)?;
            // The copy inherits WAL mode from the live database. A backup
            // should be a single self-contained file, so switch it back.
            copy.pragma_update(None, "journal_mode", "DELETE")?;
            check_database(&copy, &dest.display().to_string())?;
            drop(copy);
            std::fs::rename(&partial, dest)?;
            Ok(())
        })();
        if result.is_err() {
            remove_partial();
        }
        result
    }

    /// Replaces the whole database with the contents of the backup at `src`,
    /// then migrates it if it came from an older version. The backup is
    /// validated first; if it is unusable, nothing changes.
    pub fn restore_from(&mut self, src: &Path) -> Result<()> {
        validate_backup(src)?;
        self.conn.restore(
            rusqlite::MAIN_DB,
            src,
            None::<fn(rusqlite::backup::Progress)>,
        )?;
        self.conn.pragma_update(None, "foreign_keys", "ON")?;
        migrate(&mut self.conn)?;
        check_database(&self.conn, "the restored database")
    }

    fn discussion(&self, id: Uuid) -> Result<Discussion> {
        self.conn
            .query_row(
                "SELECT id, title, created_at, updated_at, archived_at, pinned_at
                 FROM discussions WHERE id = ?1 AND deleted_at IS NULL",
                [id.to_string()],
                discussion_from_row,
            )
            .optional()?
            .ok_or(Error::NotFound("discussion"))
    }

    fn post(&self, id: Uuid) -> Result<Post> {
        self.conn
            .query_row(
                &format!("SELECT {POST_COLUMNS} FROM posts WHERE id = ?1"),
                [id.to_string()],
                post_from_row,
            )
            .optional()?
            .ok_or(Error::NotFound("post"))
    }

    fn participant(&self, id: Uuid) -> Result<Participant> {
        self.conn
            .query_row(
                "SELECT id, kind, display_name, provider, model FROM participants WHERE id = ?1",
                [id.to_string()],
                participant_from_row,
            )
            .optional()?
            .ok_or(Error::NotFound("participant"))
    }
}

/// Checks that `conn` is an undamaged Nimata database (or a new, empty one).
fn check_database(conn: &Connection, name: &str) -> Result<()> {
    let corrupt = |e: rusqlite::Error| match e.sqlite_error_code() {
        Some(ErrorCode::NotADatabase) => Error::NotNimata(name.to_string()),
        Some(ErrorCode::DatabaseCorrupt) => Error::Corrupt(e.to_string()),
        _ => Error::Storage(e),
    };
    let result: String = conn
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .map_err(corrupt)?;
    if result != "ok" {
        return Err(Error::Corrupt(result));
    }
    let application_id: i64 = conn
        .pragma_query_value(None, "application_id", |row| row.get(0))
        .map_err(corrupt)?;
    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let has_posts: bool = conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'posts')",
        [],
        |row| row.get(0),
    )?;
    let is_empty: bool = conn.query_row(
        "SELECT NOT EXISTS (SELECT 1 FROM sqlite_master)",
        [],
        |row| row.get(0),
    )?;
    let recognised = application_id == APPLICATION_ID
        || (application_id == 0 && (is_empty || (version >= 1 && has_posts)));
    if !recognised {
        return Err(Error::NotNimata(name.to_string()));
    }
    Ok(())
}

/// Checks that `src` can be restored: a readable, undamaged Nimata database
/// that this version understands.
fn validate_backup(src: &Path) -> Result<()> {
    let name = src.display().to_string();
    let conn = Connection::open_with_flags(src, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| Error::NotNimata(name.clone()))?;
    check_database(&conn, &name)?;
    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version == 0 {
        return Err(Error::NotNimata(name));
    }
    if version > SCHEMA_VERSION {
        return Err(Error::NewerSchema {
            found: version,
            supported: SCHEMA_VERSION,
        });
    }
    Ok(())
}

fn migrate(conn: &mut Connection) -> Result<()> {
    let current: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if current > SCHEMA_VERSION {
        return Err(Error::NewerSchema {
            found: current,
            supported: SCHEMA_VERSION,
        });
    }
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", index as i64 + 1)?;
        tx.commit()?;
    }
    Ok(())
}

/// Checks that `parent_id` names a post in `discussion_id`.
fn check_parent(conn: &Connection, discussion_id: Uuid, parent_id: Uuid) -> Result<()> {
    let parent = conn
        .query_row(
            "SELECT discussion_id, deleted_at IS NOT NULL FROM posts WHERE id = ?1",
            [parent_id.to_string()],
            |row| Ok((uuid_at(row, 0)?, row.get::<_, bool>(1)?)),
        )
        .optional()?;
    match parent {
        None => Err(Error::NotFound("post being replied to")),
        Some((d, _)) if d != discussion_id => Err(Error::Invalid(
            "a post can only reply to a post in the same discussion".into(),
        )),
        Some((_, true)) => Err(Error::Invalid("a deleted post cannot be replied to".into())),
        Some(_) => Ok(()),
    }
}

/// Checks that `discussion_id` names a discussion that has not been deleted.
fn check_discussion(conn: &Connection, discussion_id: Uuid) -> Result<()> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM discussions WHERE id = ?1 AND deleted_at IS NULL)",
        [discussion_id.to_string()],
        |row| row.get(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(Error::NotFound("discussion"))
    }
}

fn insert_post(
    conn: &Connection,
    discussion_id: Uuid,
    parent_id: Option<Uuid>,
    author_id: Uuid,
    body: String,
    at: Timestamp,
) -> Result<Post> {
    let post = Post {
        id: Uuid::now_v7(),
        discussion_id,
        parent_id,
        author_id,
        body,
        created_at: at.at,
        tz_offset_minutes: at.offset_minutes,
        edited_at: None,
        deleted_at: None,
        status: PostStatus::Complete,
    };
    conn.execute(
        "INSERT INTO posts (id, discussion_id, parent_id, author_id, body, created_at,
                            tz_offset_minutes, status, modified_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?6)",
        params![
            post.id.to_string(),
            post.discussion_id.to_string(),
            post.parent_id.map(|id| id.to_string()),
            post.author_id.to_string(),
            post.body,
            post.created_at.0,
            post.tz_offset_minutes,
            status_str(post.status),
        ],
    )?;
    Ok(post)
}

impl Repository for SqliteRepository {
    fn local_user(&mut self) -> Result<Participant> {
        let existing = self
            .conn
            .query_row(
                "SELECT id, kind, display_name, provider, model
                 FROM participants WHERE is_local_user = 1",
                [],
                participant_from_row,
            )
            .optional()?;
        if let Some(user) = existing {
            return Ok(user);
        }
        let user = Participant {
            id: Uuid::now_v7(),
            kind: ParticipantKind::Human,
            display_name: LOCAL_USER_DEFAULT_NAME.into(),
            provider: None,
            model: None,
        };
        self.conn.execute(
            "INSERT INTO participants (id, kind, display_name, is_local_user)
             VALUES (?1, 'human', ?2, 1)",
            params![user.id.to_string(), user.display_name],
        )?;
        Ok(user)
    }

    fn rename_participant(&mut self, id: Uuid, display_name: &str) -> Result<Participant> {
        let name = clean_display_name(display_name)?;
        let changed = self.conn.execute(
            "UPDATE participants SET display_name = ?2 WHERE id = ?1",
            params![id.to_string(), name],
        )?;
        if changed == 0 {
            return Err(Error::NotFound("participant"));
        }
        self.participant(id)
    }

    fn start_discussion(
        &mut self,
        title: &str,
        author_id: Uuid,
        body: &str,
        at: Timestamp,
    ) -> Result<(Discussion, Post)> {
        let body = clean_body(body)?;
        let title = if title.trim().is_empty() {
            clean_title(&title_from_body(&body))?
        } else {
            clean_title(title)?
        };
        let discussion = Discussion {
            id: Uuid::now_v7(),
            title,
            created_at: at.at,
            updated_at: at.at,
            archived_at: None,
            pinned_at: None,
        };

        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO discussions (id, title, created_at, updated_at, modified_at)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            params![
                discussion.id.to_string(),
                discussion.title,
                discussion.created_at.0,
                discussion.updated_at.0
            ],
        )?;
        let post = insert_post(&tx, discussion.id, None, author_id, body, at)?;
        tx.commit()?;
        Ok((discussion, post))
    }

    fn rename_discussion(&mut self, id: Uuid, title: &str, at: UnixMillis) -> Result<Discussion> {
        let title = clean_title(title)?;
        let changed = self.conn.execute(
            "UPDATE discussions SET title = ?2, modified_at = ?3
             WHERE id = ?1 AND deleted_at IS NULL",
            params![id.to_string(), title, at.0],
        )?;
        if changed == 0 {
            return Err(Error::NotFound("discussion"));
        }
        self.discussion(id)
    }

    fn set_archived(&mut self, id: Uuid, archived: bool, at: UnixMillis) -> Result<Discussion> {
        let changed = self.conn.execute(
            "UPDATE discussions SET archived_at = ?2, modified_at = ?3
             WHERE id = ?1 AND deleted_at IS NULL",
            params![id.to_string(), archived.then_some(at.0), at.0],
        )?;
        if changed == 0 {
            return Err(Error::NotFound("discussion"));
        }
        self.discussion(id)
    }

    fn list_discussions(&mut self, filter: DiscussionFilter) -> Result<Vec<DiscussionSummary>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT d.id, d.title, d.updated_at,
                    (SELECT count(*) FROM posts p
                     WHERE p.discussion_id = d.id AND p.deleted_at IS NULL),
                    (SELECT p.body FROM posts p
                     WHERE p.discussion_id = d.id AND p.deleted_at IS NULL
                     ORDER BY p.created_at DESC, p.rowid DESC LIMIT 1)
             FROM discussions d
             WHERE (d.archived_at IS NOT NULL) = ?1 AND d.deleted_at IS NULL
             ORDER BY d.updated_at DESC, d.rowid DESC",
        )?;
        let archived = filter == DiscussionFilter::Archived;
        let rows = stmt.query_map([archived], |row| {
            let latest_body: Option<String> = row.get(4)?;
            Ok(DiscussionSummary {
                id: uuid_at(row, 0)?,
                title: row.get(1)?,
                last_activity_at: UnixMillis(row.get(2)?),
                post_count: row.get::<_, i64>(3)? as usize,
                excerpt: latest_body.map(|b| excerpt(&b, 120)).unwrap_or_default(),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    fn get_discussion(&mut self, id: Uuid) -> Result<DiscussionView> {
        let discussion = self.discussion(id)?;

        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT {POST_COLUMNS} FROM posts WHERE discussion_id = ?1 ORDER BY created_at, rowid"
        ))?;
        let posts = stmt
            .query_map([id.to_string()], post_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut stmt = self.conn.prepare_cached(
            "SELECT id, kind, display_name, provider, model FROM participants
             WHERE id IN (SELECT author_id FROM posts WHERE discussion_id = ?1)
             ORDER BY display_name",
        )?;
        let participants = stmt
            .query_map([id.to_string()], participant_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let draft = self
            .conn
            .query_row(
                "SELECT discussion_id, parent_id, body, updated_at FROM drafts
                 WHERE discussion_id = ?1",
                [id.to_string()],
                |row| {
                    Ok(Draft {
                        discussion_id: uuid_at(row, 0)?,
                        parent_id: opt_uuid_at(row, 1)?,
                        body: row.get(2)?,
                        updated_at: UnixMillis(row.get(3)?),
                    })
                },
            )
            .optional()?;

        Ok(DiscussionView {
            discussion,
            posts,
            participants,
            draft,
        })
    }

    fn add_post(
        &mut self,
        discussion_id: Uuid,
        parent_id: Option<Uuid>,
        author_id: Uuid,
        body: &str,
        at: Timestamp,
    ) -> Result<Post> {
        let body = clean_body(body)?;
        let tx = self.conn.transaction()?;
        check_discussion(&tx, discussion_id)?;
        if let Some(parent_id) = parent_id {
            check_parent(&tx, discussion_id, parent_id)?;
        }
        let post = insert_post(&tx, discussion_id, parent_id, author_id, body, at)?;
        tx.execute(
            "UPDATE discussions
             SET updated_at = max(updated_at, ?2), modified_at = max(modified_at, ?2)
             WHERE id = ?1",
            params![discussion_id.to_string(), at.at.0],
        )?;
        tx.execute(
            "DELETE FROM drafts WHERE discussion_id = ?1",
            [discussion_id.to_string()],
        )?;
        tx.commit()?;
        Ok(post)
    }

    fn edit_post(
        &mut self,
        post_id: Uuid,
        editor_id: Uuid,
        body: &str,
        at: UnixMillis,
    ) -> Result<Post> {
        let body = clean_body(body)?;
        let post = self.post(post_id)?;
        if post.author_id != editor_id {
            return Err(Error::Invalid("only the author can edit a post".into()));
        }
        if post.is_deleted() {
            return Err(Error::Invalid("a deleted post cannot be edited".into()));
        }
        if post.status != PostStatus::Complete {
            return Err(Error::Invalid(
                "a post can only be edited once it is complete".into(),
            ));
        }
        if post.body == body {
            return Ok(post);
        }
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO post_revisions (id, post_id, body, written_at, replaced_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                Uuid::now_v7().to_string(),
                post_id.to_string(),
                post.body,
                post.edited_at.unwrap_or(post.created_at).0,
                at.0
            ],
        )?;
        tx.execute(
            "UPDATE posts SET body = ?2, edited_at = ?3, modified_at = ?3 WHERE id = ?1",
            params![post_id.to_string(), body, at.0],
        )?;
        tx.commit()?;
        self.post(post_id)
    }

    fn post_revisions(&mut self, post_id: Uuid) -> Result<Vec<Revision>> {
        self.post(post_id)?;
        let mut stmt = self.conn.prepare_cached(
            "SELECT post_id, body, written_at, replaced_at FROM post_revisions
             WHERE post_id = ?1 ORDER BY replaced_at, rowid",
        )?;
        let rows = stmt.query_map([post_id.to_string()], revision_from_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    fn discussion_revisions(
        &mut self,
        discussion_id: Uuid,
    ) -> Result<HashMap<Uuid, Vec<Revision>>> {
        check_discussion(&self.conn, discussion_id)?;
        let mut stmt = self.conn.prepare_cached(
            "SELECT r.post_id, r.body, r.written_at, r.replaced_at
             FROM post_revisions r JOIN posts p ON p.id = r.post_id
             WHERE p.discussion_id = ?1 ORDER BY r.replaced_at, r.rowid",
        )?;
        let mut by_post: HashMap<Uuid, Vec<Revision>> = HashMap::new();
        for revision in stmt.query_map([discussion_id.to_string()], revision_from_row)? {
            let revision = revision?;
            by_post.entry(revision.post_id).or_default().push(revision);
        }
        Ok(by_post)
    }

    fn delete_post(&mut self, post_id: Uuid, by: Uuid, at: UnixMillis) -> Result<Post> {
        let post = self.post(post_id)?;
        if post.author_id != by {
            return Err(Error::Invalid("only the author can delete a post".into()));
        }
        if post.is_deleted() {
            return Ok(post);
        }
        let tx = self.conn.transaction()?;
        tx.execute(
            "DELETE FROM post_revisions WHERE post_id = ?1",
            [post_id.to_string()],
        )?;
        tx.execute(
            "UPDATE posts SET body = '', deleted_at = ?2, modified_at = ?2 WHERE id = ?1",
            params![post_id.to_string(), at.0],
        )?;
        // A draft can no longer reply to it; keep the draft's text.
        tx.execute(
            "UPDATE drafts SET parent_id = NULL WHERE parent_id = ?1",
            [post_id.to_string()],
        )?;
        tx.commit()?;
        self.post(post_id)
    }

    fn delete_discussion(&mut self, id: Uuid, at: UnixMillis) -> Result<()> {
        let tx = self.conn.transaction()?;
        check_discussion(&tx, id)?;
        let id = id.to_string();
        tx.execute(
            "DELETE FROM post_revisions
             WHERE post_id IN (SELECT id FROM posts WHERE discussion_id = ?1)",
            [&id],
        )?;
        tx.execute("DELETE FROM drafts WHERE discussion_id = ?1", [&id])?;
        tx.execute(
            "UPDATE posts SET body = '', deleted_at = coalesce(deleted_at, ?2), modified_at = ?2
             WHERE discussion_id = ?1",
            params![id, at.0],
        )?;
        tx.execute(
            "UPDATE discussions SET title = '', deleted_at = ?2, modified_at = ?2 WHERE id = ?1",
            params![id, at.0],
        )?;
        tx.commit()?;
        Ok(())
    }

    fn save_draft(
        &mut self,
        discussion_id: Uuid,
        parent_id: Option<Uuid>,
        body: &str,
        at: UnixMillis,
    ) -> Result<Option<Draft>> {
        if body.trim().is_empty() && parent_id.is_none() {
            self.conn.execute(
                "DELETE FROM drafts WHERE discussion_id = ?1",
                [discussion_id.to_string()],
            )?;
            return Ok(None);
        }
        check_discussion(&self.conn, discussion_id)?;
        if let Some(parent_id) = parent_id {
            check_parent(&self.conn, discussion_id, parent_id)?;
        }
        self.conn.execute(
            "INSERT INTO drafts (discussion_id, parent_id, body, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (discussion_id) DO UPDATE
             SET parent_id = excluded.parent_id, body = excluded.body,
                 updated_at = excluded.updated_at",
            params![
                discussion_id.to_string(),
                parent_id.map(|id| id.to_string()),
                body,
                at.0
            ],
        )?;
        Ok(Some(Draft {
            discussion_id,
            parent_id,
            body: body.to_string(),
            updated_at: at,
        }))
    }
}

const POST_COLUMNS: &str = "id, discussion_id, parent_id, author_id, body, created_at, \
     tz_offset_minutes, edited_at, deleted_at, status";

fn revision_from_row(row: &Row) -> rusqlite::Result<Revision> {
    Ok(Revision {
        post_id: uuid_at(row, 0)?,
        body: row.get(1)?,
        written_at: UnixMillis(row.get(2)?),
        replaced_at: UnixMillis(row.get(3)?),
    })
}

fn uuid_at(row: &Row, idx: usize) -> rusqlite::Result<Uuid> {
    let text: String = row.get(idx)?;
    Uuid::parse_str(&text).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(idx, rusqlite::types::Type::Text, e.into())
    })
}

fn opt_uuid_at(row: &Row, idx: usize) -> rusqlite::Result<Option<Uuid>> {
    match row.get::<_, Option<String>>(idx)? {
        None => Ok(None),
        Some(_) => uuid_at(row, idx).map(Some),
    }
}

fn status_str(status: PostStatus) -> &'static str {
    match status {
        PostStatus::Complete => "complete",
        PostStatus::Streaming => "streaming",
        PostStatus::Failed => "failed",
        PostStatus::Cancelled => "cancelled",
    }
}

fn parse_status(s: &str) -> rusqlite::Result<PostStatus> {
    Ok(match s {
        "complete" => PostStatus::Complete,
        "streaming" => PostStatus::Streaming,
        "failed" => PostStatus::Failed,
        "cancelled" => PostStatus::Cancelled,
        other => return Err(invalid_text(other)),
    })
}

fn parse_kind(s: &str) -> rusqlite::Result<ParticipantKind> {
    Ok(match s {
        "human" => ParticipantKind::Human,
        "model" => ParticipantKind::Model,
        "agent" => ParticipantKind::Agent,
        other => return Err(invalid_text(other)),
    })
}

fn invalid_text(value: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        format!("unexpected value {value:?}").into(),
    )
}

fn discussion_from_row(row: &Row) -> rusqlite::Result<Discussion> {
    Ok(Discussion {
        id: uuid_at(row, 0)?,
        title: row.get(1)?,
        created_at: UnixMillis(row.get(2)?),
        updated_at: UnixMillis(row.get(3)?),
        archived_at: row.get::<_, Option<i64>>(4)?.map(UnixMillis),
        pinned_at: row.get::<_, Option<i64>>(5)?.map(UnixMillis),
    })
}

fn participant_from_row(row: &Row) -> rusqlite::Result<Participant> {
    Ok(Participant {
        id: uuid_at(row, 0)?,
        kind: parse_kind(&row.get::<_, String>(1)?)?,
        display_name: row.get(2)?,
        provider: row.get(3)?,
        model: row.get(4)?,
    })
}

fn post_from_row(row: &Row) -> rusqlite::Result<Post> {
    Ok(Post {
        id: uuid_at(row, 0)?,
        discussion_id: uuid_at(row, 1)?,
        parent_id: opt_uuid_at(row, 2)?,
        author_id: uuid_at(row, 3)?,
        body: row.get(4)?,
        created_at: UnixMillis(row.get(5)?),
        tz_offset_minutes: row.get(6)?,
        edited_at: row.get::<_, Option<i64>>(7)?.map(UnixMillis),
        deleted_at: row.get::<_, Option<i64>>(8)?.map(UnixMillis),
        status: parse_status(&row.get::<_, String>(9)?)?,
    })
}

#[cfg(test)]
mod tests {
    //! Behaviour that depends on SQLite details rather than the repository
    //! contract: data written by something other than Nimata, and atomicity.

    use super::*;

    fn repo() -> SqliteRepository {
        SqliteRepository::open_in_memory().unwrap()
    }

    fn at(minutes: i64) -> Timestamp {
        Timestamp {
            at: UnixMillis(1_791_132_067_123 + minutes * 60_000),
            offset_minutes: 0,
        }
    }

    #[test]
    fn a_failed_first_post_leaves_no_discussion_behind() {
        let mut repo = repo();
        // The author does not exist, so the post insert violates a foreign
        // key after the discussion row was written in the same transaction.
        let result = repo.start_discussion("Orphan", Uuid::now_v7(), "body", at(0));
        assert!(matches!(result, Err(Error::Storage(_))));
        assert!(
            repo.list_discussions(DiscussionFilter::Active)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_failed_post_keeps_the_draft_and_activity_time() {
        let mut repo = repo();
        let me = repo.local_user().unwrap();
        let (d, root) = repo.start_discussion("D", me.id, "root", at(0)).unwrap();
        repo.save_draft(d.id, Some(root.id), "keep me", UnixMillis(0))
            .unwrap();

        let result = repo.add_post(d.id, Some(root.id), Uuid::now_v7(), "x", at(5));
        assert!(matches!(result, Err(Error::Storage(_))));

        let view = repo.get_discussion(d.id).unwrap();
        assert_eq!(view.posts.len(), 1);
        assert_eq!(view.draft.unwrap().body, "keep me");
        assert_eq!(view.discussion.updated_at, at(0).at);
    }

    #[test]
    fn every_post_status_round_trips() {
        let mut repo = repo();
        let me = repo.local_user().unwrap();
        let (d, root) = repo.start_discussion("D", me.id, "root", at(0)).unwrap();
        for status in ["streaming", "failed", "cancelled", "complete"] {
            repo.conn
                .execute(
                    "UPDATE posts SET status = ?1 WHERE id = ?2",
                    params![status, root.id.to_string()],
                )
                .unwrap();
            let read = repo.get_discussion(d.id).unwrap().posts[0].status;
            assert_eq!(status_str(read), status);
        }
    }

    #[test]
    fn model_and_agent_participants_are_read_back() {
        let repo = repo();
        for (kind, expected) in [
            ("model", ParticipantKind::Model),
            ("agent", ParticipantKind::Agent),
        ] {
            let id = Uuid::now_v7();
            repo.conn
                .execute(
                    "INSERT INTO participants (id, kind, display_name, provider, model)
                     VALUES (?1, ?2, 'P', 'openai', 'gpt')",
                    params![id.to_string(), kind],
                )
                .unwrap();
            let p = repo.participant(id).unwrap();
            assert_eq!((p.kind, p.provider.as_deref()), (expected, Some("openai")));
        }
    }

    #[test]
    fn unexpected_stored_values_are_errors_not_panics() {
        let mut repo = repo();
        let me = repo.local_user().unwrap();
        let (d, root) = repo.start_discussion("D", me.id, "root", at(0)).unwrap();

        // Bypass the CHECK constraint the way another tool might.
        repo.conn
            .execute_batch("PRAGMA ignore_check_constraints = ON")
            .unwrap();
        repo.conn
            .execute(
                "UPDATE posts SET status = 'sideways' WHERE id = ?1",
                [root.id.to_string()],
            )
            .unwrap();
        assert!(matches!(repo.get_discussion(d.id), Err(Error::Storage(_))));

        repo.conn
            .execute_batch("PRAGMA foreign_keys = OFF")
            .unwrap();
        repo.conn
            .execute("UPDATE posts SET status = 'complete'", [])
            .unwrap();
        // Rows cannot be re-parented (a trigger forbids it), so insert one.
        repo.conn
            .execute(
                "INSERT INTO posts (id, discussion_id, parent_id, author_id, body, created_at,
                                    tz_offset_minutes, status)
                 SELECT 'bad', discussion_id, 'not-a-uuid', author_id, 'x', 1, 0, 'complete'
                 FROM posts",
                [],
            )
            .unwrap();
        assert!(matches!(repo.get_discussion(d.id), Err(Error::Storage(_))));

        repo.conn
            .execute("UPDATE participants SET kind = 'robot'", [])
            .unwrap();
        assert!(matches!(repo.local_user(), Err(Error::Storage(_))));
    }

    #[test]
    fn missing_records_are_reported_as_not_found() {
        let mut repo = repo();
        let me = repo.local_user().unwrap();
        let (d, _) = repo.start_discussion("D", me.id, "root", at(0)).unwrap();
        let (_, other_root) = repo.start_discussion("E", me.id, "other", at(1)).unwrap();
        let nobody = Uuid::now_v7();

        assert!(matches!(
            repo.rename_participant(nobody, "X"),
            Err(Error::NotFound(_))
        ));
        assert!(matches!(
            repo.set_archived(nobody, true, UnixMillis(0)),
            Err(Error::NotFound(_))
        ));
        assert!(matches!(
            repo.get_discussion(nobody),
            Err(Error::NotFound(_))
        ));
        assert!(matches!(
            repo.save_draft(nobody, None, "text", UnixMillis(0)),
            Err(Error::NotFound(_))
        ));
        assert!(matches!(
            repo.save_draft(d.id, Some(other_root.id), "text", UnixMillis(0)),
            Err(Error::Invalid(_))
        ));
    }

    #[test]
    fn error_messages_are_readable() {
        assert_eq!(
            Error::NotFound("discussion").to_string(),
            "discussion not found"
        );
        assert_eq!(
            Error::NewerSchema {
                found: 3,
                supported: 1
            }
            .to_string(),
            "this database was created by a newer version of Nimata (schema 3, supported 1)"
        );
    }

    #[test]
    fn creation_facts_cannot_be_rewritten_even_with_raw_sql() {
        let mut repo = repo();
        let me = repo.local_user().unwrap();
        let (d, root) = repo.start_discussion("D", me.id, "root", at(0)).unwrap();
        repo.edit_post(root.id, me.id, "edited", UnixMillis(5))
            .unwrap();

        for sql in [
            "UPDATE posts SET created_at = 0",
            "UPDATE posts SET tz_offset_minutes = 60",
            "UPDATE posts SET parent_id = id",
            "UPDATE posts SET author_id = 'someone'",
            "UPDATE discussions SET created_at = 0",
            "UPDATE post_revisions SET body = 'rewritten'",
        ] {
            let error = repo.conn.execute(sql, []).unwrap_err().to_string();
            assert!(error.contains("cannot"), "{sql}: {error}");
        }
        // Ordinary changes still work.
        repo.conn
            .execute(
                "UPDATE discussions SET title = 'T' WHERE id = ?1",
                [d.id.to_string()],
            )
            .unwrap();
    }

    mod files {
        use super::*;
        use tempfile::TempDir;

        fn populated(dir: &TempDir) -> (SqliteRepository, Uuid) {
            let mut repo = SqliteRepository::open(&dir.path().join("live.sqlite3")).unwrap();
            let me = repo.local_user().unwrap();
            let (d, root) = repo.start_discussion("Kept", me.id, "root", at(0)).unwrap();
            repo.add_post(d.id, Some(root.id), me.id, "reply", at(1))
                .unwrap();
            (repo, d.id)
        }

        #[test]
        fn a_backup_restores_the_same_discussions() {
            let dir = TempDir::new().unwrap();
            let (mut repo, id) = populated(&dir);
            let before = repo.get_discussion(id).unwrap();
            let backup = dir.path().join("backup.sqlite3");
            repo.backup_to(&backup).unwrap();
            let mut files: Vec<_> = std::fs::read_dir(dir.path())
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .filter(|name| name.starts_with("backup"))
                .collect();
            files.sort();
            assert_eq!(
                files,
                vec!["backup.sqlite3"],
                "a backup is one self-contained file"
            );

            // Change things after the backup, then restore it.
            let me = repo.local_user().unwrap();
            repo.start_discussion("Later", me.id, "after the backup", at(9))
                .unwrap();
            repo.delete_discussion(id, UnixMillis(10)).unwrap();
            repo.restore_from(&backup).unwrap();

            assert_eq!(repo.get_discussion(id).unwrap(), before);
            let titles: Vec<_> = repo
                .list_discussions(DiscussionFilter::Active)
                .unwrap()
                .into_iter()
                .map(|s| s.title)
                .collect();
            assert_eq!(titles, vec!["Kept"]);
        }

        #[test]
        fn a_backup_restores_into_a_brand_new_database() {
            let dir = TempDir::new().unwrap();
            let (repo, id) = populated(&dir);
            let backup = dir.path().join("backup.sqlite3");
            repo.backup_to(&backup).unwrap();
            drop(repo);

            let mut fresh = SqliteRepository::open(&dir.path().join("fresh.sqlite3")).unwrap();
            fresh.restore_from(&backup).unwrap();
            assert_eq!(fresh.get_discussion(id).unwrap().posts.len(), 2);
        }

        #[test]
        fn unusable_backups_are_refused_and_nothing_changes() {
            let dir = TempDir::new().unwrap();
            let (mut repo, id) = populated(&dir);

            let text = dir.path().join("notes.txt");
            std::fs::write(&text, "not a database at all").unwrap();
            let other = dir.path().join("other.sqlite3");
            Connection::open(&other)
                .unwrap()
                .execute_batch("CREATE TABLE photos (id INTEGER); INSERT INTO photos VALUES (1);")
                .unwrap();
            let empty = dir.path().join("empty.sqlite3");
            Connection::open(&empty)
                .unwrap()
                .execute_batch("VACUUM")
                .unwrap();
            let newer = dir.path().join("newer.sqlite3");
            repo.backup_to(&newer).unwrap();
            Connection::open(&newer)
                .unwrap()
                .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
                .unwrap();

            for (file, expected) in [
                (&text, "not a Nimata database"),
                (&other, "not a Nimata database"),
                (&empty, "not a Nimata database"),
                (&newer, "newer version"),
                (&dir.path().join("missing.sqlite3"), "not a Nimata database"),
            ] {
                let error = repo.restore_from(file).unwrap_err().to_string();
                assert!(error.contains(expected), "{}: {error}", file.display());
            }
            assert_eq!(repo.get_discussion(id).unwrap().posts.len(), 2);
        }

        #[test]
        fn opening_a_damaged_or_foreign_file_is_refused() {
            let dir = TempDir::new().unwrap();
            let text = dir.path().join("notes.txt");
            std::fs::write(&text, "x".repeat(4096)).unwrap();
            assert!(matches!(
                SqliteRepository::open(&text),
                Err(Error::NotNimata(_))
            ));

            let other = dir.path().join("other.sqlite3");
            Connection::open(&other)
                .unwrap()
                .execute_batch("CREATE TABLE photos (id INTEGER);")
                .unwrap();
            assert!(matches!(
                SqliteRepository::open(&other),
                Err(Error::NotNimata(_))
            ));
            let tables: i64 = Connection::open(&other)
                .unwrap()
                .query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get(0))
                .unwrap();
            assert_eq!(tables, 1, "a foreign database is left untouched");
        }

        #[test]
        fn a_damaged_database_is_detected_on_open() {
            let dir = TempDir::new().unwrap();
            let path = dir.path().join("live.sqlite3");
            {
                let mut repo = SqliteRepository::open(&path).unwrap();
                let me = repo.local_user().unwrap();
                for n in 0..200 {
                    repo.start_discussion("D", me.id, &"words ".repeat(50), at(n))
                        .unwrap();
                }
                repo.conn
                    .pragma_update(None, "journal_mode", "DELETE")
                    .unwrap();
            }
            // Overwrite the middle of the file, as a failing disk might.
            let mut bytes = std::fs::read(&path).unwrap();
            let middle = bytes.len() / 2;
            bytes[middle..middle + 4096].fill(0xAB);
            std::fs::write(&path, bytes).unwrap();

            match SqliteRepository::open(&path) {
                Err(Error::Corrupt(_)) => {}
                Err(other) => panic!("expected Corrupt, got {other:?}"),
                Ok(_) => panic!("a damaged database was opened"),
            }
        }
    }
}
