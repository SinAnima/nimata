//! SQLite implementation of [`Repository`].
//!
//! IDs are stored as hyphenated UUID text and times as integers so the
//! database stays readable with ordinary SQLite tools.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, Row, params};
use uuid::Uuid;

use crate::domain::{
    Discussion, DiscussionFilter, DiscussionSummary, DiscussionView, Draft, Participant,
    ParticipantKind, Post, PostStatus, clean_body, clean_display_name, clean_title, excerpt,
    title_from_body,
};
use crate::error::{Error, Result};
use crate::repository::Repository;
use crate::time::{Timestamp, UnixMillis};

/// Schema migrations, applied in order. The schema version is the number
/// applied, recorded in `PRAGMA user_version`.
const MIGRATIONS: &[&str] = &[include_str!("../migrations/0001_initial.sql")];

pub const SCHEMA_VERSION: i64 = MIGRATIONS.len() as i64;

const LOCAL_USER_DEFAULT_NAME: &str = "Me";

pub struct SqliteRepository {
    conn: Connection,
}

impl SqliteRepository {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
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

    fn discussion(&self, id: Uuid) -> Result<Discussion> {
        self.conn
            .query_row(
                "SELECT id, title, created_at, updated_at, archived_at, pinned_at
                 FROM discussions WHERE id = ?1",
                [id.to_string()],
                discussion_from_row,
            )
            .optional()?
            .ok_or(Error::NotFound("discussion"))
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
    let parent_discussion = conn
        .query_row(
            "SELECT discussion_id FROM posts WHERE id = ?1",
            [parent_id.to_string()],
            |row| uuid_at(row, 0),
        )
        .optional()?;
    match parent_discussion {
        None => Err(Error::NotFound("post being replied to")),
        Some(d) if d != discussion_id => Err(Error::Invalid(
            "a post can only reply to a post in the same discussion".into(),
        )),
        Some(_) => Ok(()),
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
        status: PostStatus::Complete,
    };
    conn.execute(
        "INSERT INTO posts (id, discussion_id, parent_id, author_id, body, created_at,
                            tz_offset_minutes, status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
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
            "INSERT INTO discussions (id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
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

    fn rename_discussion(&mut self, id: Uuid, title: &str) -> Result<Discussion> {
        let title = clean_title(title)?;
        let changed = self.conn.execute(
            "UPDATE discussions SET title = ?2 WHERE id = ?1",
            params![id.to_string(), title],
        )?;
        if changed == 0 {
            return Err(Error::NotFound("discussion"));
        }
        self.discussion(id)
    }

    fn set_archived(&mut self, id: Uuid, archived: bool, at: UnixMillis) -> Result<Discussion> {
        let changed = self.conn.execute(
            "UPDATE discussions SET archived_at = ?2 WHERE id = ?1",
            params![id.to_string(), archived.then_some(at.0)],
        )?;
        if changed == 0 {
            return Err(Error::NotFound("discussion"));
        }
        self.discussion(id)
    }

    fn list_discussions(&mut self, filter: DiscussionFilter) -> Result<Vec<DiscussionSummary>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT d.id, d.title, d.updated_at,
                    (SELECT count(*) FROM posts p WHERE p.discussion_id = d.id),
                    (SELECT p.body FROM posts p WHERE p.discussion_id = d.id
                     ORDER BY p.created_at DESC, p.rowid DESC LIMIT 1)
             FROM discussions d
             WHERE (d.archived_at IS NOT NULL) = ?1
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

        let mut stmt = self.conn.prepare_cached(
            "SELECT id, discussion_id, parent_id, author_id, body, created_at,
                    tz_offset_minutes, edited_at, status
             FROM posts WHERE discussion_id = ?1
             ORDER BY created_at, rowid",
        )?;
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
        let exists: bool = tx.query_row(
            "SELECT EXISTS (SELECT 1 FROM discussions WHERE id = ?1)",
            [discussion_id.to_string()],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(Error::NotFound("discussion"));
        }
        if let Some(parent_id) = parent_id {
            check_parent(&tx, discussion_id, parent_id)?;
        }
        let post = insert_post(&tx, discussion_id, parent_id, author_id, body, at)?;
        tx.execute(
            "UPDATE discussions SET updated_at = max(updated_at, ?2) WHERE id = ?1",
            params![discussion_id.to_string(), at.at.0],
        )?;
        tx.execute(
            "DELETE FROM drafts WHERE discussion_id = ?1",
            [discussion_id.to_string()],
        )?;
        tx.commit()?;
        Ok(post)
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
        self.discussion(discussion_id)?;
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
        status: parse_status(&row.get::<_, String>(8)?)?,
    })
}
