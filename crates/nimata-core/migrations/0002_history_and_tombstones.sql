-- Nimata schema version 2: edit history, tombstones, sync bookkeeping, and
-- attachment metadata.

-- Identifies the file as a Nimata database ("NIMA" in ASCII).
PRAGMA application_id = 1313426753;

-- Tombstones: a deleted record keeps its ID and structure so that other
-- devices can learn about the deletion later; its content is erased.
ALTER TABLE discussions ADD COLUMN deleted_at INTEGER;
ALTER TABLE posts ADD COLUMN deleted_at INTEGER;

-- When the record last changed in any way. Used by future sync.
ALTER TABLE discussions ADD COLUMN modified_at INTEGER NOT NULL DEFAULT 0;
UPDATE discussions SET modified_at = max(updated_at, coalesce(archived_at, 0));
ALTER TABLE posts ADD COLUMN modified_at INTEGER NOT NULL DEFAULT 0;
UPDATE posts SET modified_at = coalesce(edited_at, created_at);

DROP INDEX discussions_by_activity;
CREATE INDEX discussions_visible_by_activity
    ON discussions (archived_at IS NOT NULL, updated_at DESC)
    WHERE deleted_at IS NULL;

-- Earlier versions of edited posts. A row is the text a post had from
-- written_at until an edit replaced it at replaced_at.
CREATE TABLE post_revisions (
    id          TEXT PRIMARY KEY,
    post_id     TEXT NOT NULL REFERENCES posts (id),
    body        TEXT NOT NULL,
    written_at  INTEGER NOT NULL,
    replaced_at INTEGER NOT NULL
);

CREATE INDEX post_revisions_by_post ON post_revisions (post_id, replaced_at);

CREATE TABLE attachments (
    id           TEXT PRIMARY KEY,
    post_id      TEXT NOT NULL REFERENCES posts (id),
    filename     TEXT NOT NULL,
    media_type   TEXT NOT NULL,
    size         INTEGER NOT NULL CHECK (size >= 0),
    content_hash TEXT NOT NULL,
    local_uri    TEXT,
    created_at   INTEGER NOT NULL,
    deleted_at   INTEGER
);

CREATE INDEX attachments_by_post ON attachments (post_id);
CREATE INDEX attachments_by_hash ON attachments (content_hash);

-- History is not rewritten: what a post is, where it sits in the thread, and
-- when it was written are fixed once it exists.
CREATE TRIGGER posts_creation_is_immutable
BEFORE UPDATE OF id, discussion_id, parent_id, author_id, created_at, tz_offset_minutes ON posts
WHEN OLD.id IS NOT NEW.id
  OR OLD.discussion_id IS NOT NEW.discussion_id
  OR OLD.parent_id IS NOT NEW.parent_id
  OR OLD.author_id IS NOT NEW.author_id
  OR OLD.created_at IS NOT NEW.created_at
  OR OLD.tz_offset_minutes IS NOT NEW.tz_offset_minutes
BEGIN
    SELECT RAISE(ABORT, 'a post''s identity, thread position, and creation time cannot change');
END;

CREATE TRIGGER discussions_creation_is_immutable
BEFORE UPDATE OF id, created_at ON discussions
WHEN OLD.id IS NOT NEW.id OR OLD.created_at IS NOT NEW.created_at
BEGIN
    SELECT RAISE(ABORT, 'a discussion''s identity and creation time cannot change');
END;

-- Revisions are a record of what was said; they can be erased with their
-- post, but never altered.
CREATE TRIGGER post_revisions_are_immutable
BEFORE UPDATE ON post_revisions
BEGIN
    SELECT RAISE(ABORT, 'post revisions cannot be changed');
END;
