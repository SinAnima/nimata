-- A Nimata database at schema version 4: a model with aliases, a default
-- model, and a draft. Frozen: never regenerate it from current code.
PRAGMA application_id = 1313426753;
PRAGMA user_version = 4;
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
, provider_id TEXT REFERENCES providers (id), enabled INTEGER NOT NULL DEFAULT 1, aliases TEXT NOT NULL DEFAULT '[]');
INSERT INTO participants VALUES('01a10969-fa3e-754f-84da-3849ad072024','human','Thanos',NULL,NULL,NULL,1,NULL,1,'[]');
INSERT INTO participants VALUES('01a109a1-0000-7000-8000-000000000001','model','GPT-6-sol','openai','gpt-6-sol',NULL,0,'01a109a0-0000-7000-8000-000000000001',1,'["review","r"]');
CREATE TABLE discussions (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    archived_at INTEGER,
    pinned_at   INTEGER
, deleted_at INTEGER, modified_at INTEGER NOT NULL DEFAULT 0);
INSERT INTO discussions VALUES('01a109a2-0000-7000-8000-000000000001','Mentions',1791300000000,1791300060000,NULL,NULL,NULL,1791300060000);
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
INSERT INTO posts VALUES('01a109a3-0000-7000-8000-000000000001','01a109a2-0000-7000-8000-000000000001',NULL,'01a10969-fa3e-754f-84da-3849ad072024','@review what do you think?',1791300000000,-240,NULL,'complete',NULL,NULL,1791300000000);
CREATE TABLE drafts (
    discussion_id TEXT PRIMARY KEY REFERENCES discussions (id),
    parent_id     TEXT REFERENCES posts (id),
    body          TEXT NOT NULL,
    updated_at    INTEGER NOT NULL
);
INSERT INTO drafts VALUES('01a109a2-0000-7000-8000-000000000001','01a109a3-0000-7000-8000-000000000001','Half a thought',1791300060000);
CREATE TABLE post_revisions (
    id          TEXT PRIMARY KEY,
    post_id     TEXT NOT NULL REFERENCES posts (id),
    body        TEXT NOT NULL,
    written_at  INTEGER NOT NULL,
    replaced_at INTEGER NOT NULL
);
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
CREATE TABLE providers (
    id           TEXT PRIMARY KEY,
    kind         TEXT NOT NULL CHECK (kind IN ('openai', 'anthropic', 'openai_compatible')),
    display_name TEXT NOT NULL,
    base_url     TEXT,
    created_at   INTEGER NOT NULL,
    modified_at  INTEGER NOT NULL
);
INSERT INTO providers VALUES('01a109a0-0000-7000-8000-000000000001','openai','OpenAI',NULL,1791300000000,1791300000000);
CREATE TABLE generations (
    id               TEXT PRIMARY KEY,
    post_id          TEXT NOT NULL REFERENCES posts (id),
    participant_id   TEXT NOT NULL REFERENCES participants (id),
    status           TEXT NOT NULL
                     CHECK (status IN ('queued', 'sending', 'streaming', 'complete', 'failed', 'cancelled')),
    error            TEXT,
    context_post_ids TEXT NOT NULL,
    started_at       INTEGER NOT NULL,
    finished_at      INTEGER
);
CREATE TABLE app_settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
INSERT INTO app_settings VALUES('default_model','01a109a1-0000-7000-8000-000000000001');
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
CREATE UNIQUE INDEX participants_one_per_model
    ON participants (provider_id, model)
    WHERE provider_id IS NOT NULL;
CREATE INDEX generations_by_post ON generations (post_id);
CREATE INDEX generations_unfinished
    ON generations (status)
    WHERE status IN ('queued', 'sending', 'streaming');
COMMIT;
