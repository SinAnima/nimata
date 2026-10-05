-- A Nimata database at schema version 2 (Stage 2): an edited post with its
-- revision, a deleted post with a reply, and a deleted discussion. Frozen:
-- never regenerate it from current code.
PRAGMA application_id = 1313426753;
PRAGMA user_version = 2;
PRAGMA foreign_keys=OFF;
BEGIN TRANSACTION;
CREATE TABLE participants (
    id                 TEXT PRIMARY KEY,
    kind               TEXT NOT NULL CHECK (kind IN ('human', 'model', 'agent')),
    display_name       TEXT NOT NULL,
    provider           TEXT,
    model              TEXT,
    configuration_json TEXT,
    is_local_user      INTEGER NOT NULL DEFAULT 0
);
INSERT INTO participants VALUES('01a10969-fa3e-754f-84da-3849ad072024','human','Thanos',NULL,NULL,NULL,1);
CREATE TABLE discussions (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    archived_at INTEGER,
    pinned_at   INTEGER
, deleted_at INTEGER, modified_at INTEGER NOT NULL DEFAULT 0);
INSERT INTO discussions VALUES('01a10980-0000-7000-8000-000000000001','Datalog for the mapping engine',1791132067123,1791132307123,NULL,NULL,NULL,1791132400000);
INSERT INTO discussions VALUES('01a10980-0000-7000-8000-000000000002','',1791000000000,1791000000000,NULL,NULL,1791100000000,1791100000000);
CREATE TABLE posts (
    id                     TEXT PRIMARY KEY,
    discussion_id          TEXT NOT NULL REFERENCES discussions (id),
    parent_id              TEXT REFERENCES posts (id),
    author_id              TEXT NOT NULL REFERENCES participants (id),
    body                   TEXT NOT NULL,
    created_at             INTEGER NOT NULL,
    tz_offset_minutes      INTEGER NOT NULL,
    edited_at              INTEGER,
    status                 TEXT NOT NULL CHECK (status IN ('complete', 'streaming', 'failed', 'cancelled')),
    provider_metadata_json TEXT
, deleted_at INTEGER, modified_at INTEGER NOT NULL DEFAULT 0);
INSERT INTO posts VALUES('01a10981-0000-7000-8000-000000000001','01a10980-0000-7000-8000-000000000001',NULL,'01a10969-fa3e-754f-84da-3849ad072024','Could Datalog replace our mapping engine?',1791132067123,-240,1791132400000,'complete',NULL,NULL,1791132400000);
INSERT INTO posts VALUES('01a10981-0000-7000-8000-000000000002','01a10980-0000-7000-8000-000000000001','01a10981-0000-7000-8000-000000000001','01a10969-fa3e-754f-84da-3849ad072024','',1791132187123,180,NULL,'complete',NULL,1791132300000,1791132300000);
INSERT INTO posts VALUES('01a10981-0000-7000-8000-000000000003','01a10980-0000-7000-8000-000000000001','01a10981-0000-7000-8000-000000000002','01a10969-fa3e-754f-84da-3849ad072024','Replying to a post that was later deleted.',1791132307123,180,NULL,'complete',NULL,NULL,1791132307123);
CREATE TABLE drafts (
    discussion_id TEXT PRIMARY KEY REFERENCES discussions (id),
    parent_id     TEXT REFERENCES posts (id),
    body          TEXT NOT NULL,
    updated_at    INTEGER NOT NULL
);
CREATE TABLE post_revisions (
    id          TEXT PRIMARY KEY,
    post_id     TEXT NOT NULL REFERENCES posts (id),
    body        TEXT NOT NULL,
    written_at  INTEGER NOT NULL,
    replaced_at INTEGER NOT NULL
);
INSERT INTO post_revisions VALUES('01a10982-0000-7000-8000-000000000001','01a10981-0000-7000-8000-000000000001','Could Datalog replace the mapping engine?',1791132067123,1791132400000);
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
CREATE TRIGGER post_revisions_are_immutable
BEFORE UPDATE ON post_revisions
BEGIN
    SELECT RAISE(ABORT, 'post revisions cannot be changed');
END;
CREATE UNIQUE INDEX participants_local_user ON participants (is_local_user) WHERE is_local_user = 1;
CREATE INDEX posts_by_discussion ON posts (discussion_id, created_at);
CREATE INDEX posts_by_parent ON posts (parent_id);
CREATE INDEX discussions_visible_by_activity
    ON discussions (archived_at IS NOT NULL, updated_at DESC)
    WHERE deleted_at IS NULL;
CREATE INDEX post_revisions_by_post ON post_revisions (post_id, replaced_at);
CREATE INDEX attachments_by_post ON attachments (post_id);
CREATE INDEX attachments_by_hash ON attachments (content_hash);
COMMIT;
