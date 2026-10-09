-- A Nimata database at schema version 6: a post, a draft, and a recent
-- search, created before attachments were stored. Frozen: never regenerate
-- it from current code.
PRAGMA application_id = 1313426753;
PRAGMA user_version = 6;
/* WARNING: Script requires that SQLITE_DBCONFIG_DEFENSIVE be disabled */
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
CREATE TABLE discussions (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    archived_at INTEGER,
    pinned_at   INTEGER
, deleted_at INTEGER, modified_at INTEGER NOT NULL DEFAULT 0);
INSERT INTO discussions VALUES('01a109b0-0000-7000-8000-000000000006','Νήματα and drafts',1791500000000,1791500100000,NULL,NULL,NULL,1791500100000);
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
INSERT INTO posts VALUES('01a109b1-0000-7000-8000-000000000061','01a109b0-0000-7000-8000-000000000006',NULL,'01a10969-fa3e-754f-84da-3849ad072024','Threads, before attachments.',1791500000000,180,NULL,'complete',NULL,NULL,1791500000000);
CREATE TABLE drafts (
    discussion_id TEXT PRIMARY KEY REFERENCES discussions (id),
    parent_id     TEXT REFERENCES posts (id),
    body          TEXT NOT NULL,
    updated_at    INTEGER NOT NULL
, context_ids TEXT NOT NULL DEFAULT '[]');
INSERT INTO drafts VALUES('01a109b0-0000-7000-8000-000000000006','01a109b1-0000-7000-8000-000000000061','An unsent reply.',1791500100000,'[]');
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
, sent_json TEXT);
CREATE TABLE app_settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE context_refs (
    post_id     TEXT NOT NULL REFERENCES posts (id),
    ref_post_id TEXT NOT NULL REFERENCES posts (id),
    position    INTEGER NOT NULL,
    PRIMARY KEY (post_id, ref_post_id)
);
PRAGMA writable_schema=ON;
INSERT INTO sqlite_schema(type,name,tbl_name,rootpage,sql)VALUES('table','posts_fts','posts_fts',0,'CREATE VIRTUAL TABLE posts_fts USING fts5(
    body,
    content = '''',
    contentless_delete = 1,
    prefix = ''2 3'',
    tokenize = ''unicode61 remove_diacritics 2''
)');
CREATE TABLE IF NOT EXISTS 'posts_fts_data'(id INTEGER PRIMARY KEY, block BLOB);
INSERT INTO posts_fts_data VALUES(1,X'0103');
INSERT INTO posts_fts_data VALUES(10,X'00000000ff00000101010100010101010101000001');
INSERT INTO posts_fts_data VALUES(137438953473,X'0000005a0c306174746163686d656e747301020401066265666f7265010203010774687265616473010202000331617401020401026265010203010274680102020004326174740102040103626566010203010374687201020204100b0c0807070908');
CREATE TABLE IF NOT EXISTS 'posts_fts_idx'(segid, term, pgno, PRIMARY KEY(segid, term)) WITHOUT ROWID;
INSERT INTO posts_fts_idx VALUES(1,X'',2);
CREATE TABLE IF NOT EXISTS 'posts_fts_docsize'(id INTEGER PRIMARY KEY, sz BLOB, origin INTEGER);
INSERT INTO posts_fts_docsize VALUES(1,X'03',1);
CREATE TABLE IF NOT EXISTS 'posts_fts_config'(k PRIMARY KEY, v) WITHOUT ROWID;
INSERT INTO posts_fts_config VALUES('version',4);
INSERT INTO sqlite_schema(type,name,tbl_name,rootpage,sql)VALUES('table','discussions_fts','discussions_fts',0,'CREATE VIRTUAL TABLE discussions_fts USING fts5(
    title,
    content = '''',
    contentless_delete = 1,
    tokenize = ''unicode61 remove_diacritics 2''
)');
CREATE TABLE IF NOT EXISTS 'discussions_fts_data'(id INTEGER PRIMARY KEY, block BLOB);
INSERT INTO discussions_fts_data VALUES(1,X'0103');
INSERT INTO discussions_fts_data VALUES(10,X'00000000ff00000101010100010101010101000001');
INSERT INTO discussions_fts_data VALUES(137438953473,X'000000280430616e640102030106647261667473010204010ccebdceb7cebcceb1cf84ceb101020204080b');
CREATE TABLE IF NOT EXISTS 'discussions_fts_idx'(segid, term, pgno, PRIMARY KEY(segid, term)) WITHOUT ROWID;
INSERT INTO discussions_fts_idx VALUES(1,X'',2);
CREATE TABLE IF NOT EXISTS 'discussions_fts_docsize'(id INTEGER PRIMARY KEY, sz BLOB, origin INTEGER);
INSERT INTO discussions_fts_docsize VALUES(1,X'03',1);
CREATE TABLE IF NOT EXISTS 'discussions_fts_config'(k PRIMARY KEY, v) WITHOUT ROWID;
INSERT INTO discussions_fts_config VALUES('version',4);
CREATE TABLE recent_searches (
    query        TEXT PRIMARY KEY,
    last_used_at INTEGER NOT NULL
);
INSERT INTO recent_searches VALUES('νηματα',1791500200000);
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
CREATE TRIGGER context_refs_are_immutable
BEFORE UPDATE ON context_refs
BEGIN
    SELECT RAISE(ABORT, 'context references cannot change once a post exists');
END;
CREATE TRIGGER posts_fts_insert AFTER INSERT ON posts BEGIN
    INSERT INTO posts_fts (rowid, body) VALUES (new.rowid, nimata_fold(new.body));
END;
CREATE TRIGGER posts_fts_delete AFTER DELETE ON posts BEGIN
    DELETE FROM posts_fts WHERE rowid = old.rowid;
END;
CREATE TRIGGER posts_fts_update AFTER UPDATE OF body ON posts BEGIN
    DELETE FROM posts_fts WHERE rowid = old.rowid;
    INSERT INTO posts_fts (rowid, body) VALUES (new.rowid, nimata_fold(new.body));
END;
CREATE TRIGGER discussions_fts_insert AFTER INSERT ON discussions BEGIN
    INSERT INTO discussions_fts (rowid, title) VALUES (new.rowid, nimata_fold(new.title));
END;
CREATE TRIGGER discussions_fts_delete AFTER DELETE ON discussions BEGIN
    DELETE FROM discussions_fts WHERE rowid = old.rowid;
END;
CREATE TRIGGER discussions_fts_update AFTER UPDATE OF title ON discussions BEGIN
    DELETE FROM discussions_fts WHERE rowid = old.rowid;
    INSERT INTO discussions_fts (rowid, title) VALUES (new.rowid, nimata_fold(new.title));
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
CREATE INDEX context_refs_by_ref ON context_refs (ref_post_id);
CREATE INDEX posts_by_author ON posts (author_id, created_at);
CREATE INDEX posts_by_time ON posts (created_at);
PRAGMA writable_schema=OFF;
COMMIT;
