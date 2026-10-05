-- A Nimata database at schema version 1 (Stage 1), with representative
-- data. Frozen: never regenerate it from current code. Migration tests
-- load it and open it with the current version.
PRAGMA user_version = 1;
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
);
INSERT INTO discussions VALUES('01a10970-0000-7000-8000-000000000001','Should Nimata use CouchDB?',1791132067123,1791132307123,NULL,NULL);
INSERT INTO discussions VALUES('01a10970-0000-7000-8000-000000000002','Archived thoughts',1791000000000,1791000000000,1791100000000,NULL);
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
);
INSERT INTO posts VALUES('01a10971-0000-7000-8000-000000000001','01a10970-0000-7000-8000-000000000001',NULL,'01a10969-fa3e-754f-84da-3849ad072024','Should Nimata use CouchDB?',1791132067123,-240,NULL,'complete',NULL);
INSERT INTO posts VALUES('01a10971-0000-7000-8000-000000000002','01a10970-0000-7000-8000-000000000001','01a10971-0000-7000-8000-000000000001','01a10969-fa3e-754f-84da-3849ad072024','The attraction is replication.',1791132187123,180,NULL,'complete',NULL);
INSERT INTO posts VALUES('01a10971-0000-7000-8000-000000000003','01a10970-0000-7000-8000-000000000001','01a10971-0000-7000-8000-000000000002','01a10969-fa3e-754f-84da-3849ad072024','But mobile changes the constraints.',1791132307123,345,NULL,'complete',NULL);
INSERT INTO posts VALUES('01a10971-0000-7000-8000-000000000004','01a10970-0000-7000-8000-000000000002',NULL,'01a10969-fa3e-754f-84da-3849ad072024','An archived note.',1791000000000,0,NULL,'complete',NULL);
CREATE TABLE drafts (
    discussion_id TEXT PRIMARY KEY REFERENCES discussions (id),
    parent_id     TEXT REFERENCES posts (id),
    body          TEXT NOT NULL,
    updated_at    INTEGER NOT NULL
);
INSERT INTO drafts VALUES('01a10970-0000-7000-8000-000000000001','01a10971-0000-7000-8000-000000000003','Half a thought',1791132400000);
CREATE UNIQUE INDEX participants_local_user ON participants (is_local_user) WHERE is_local_user = 1;
CREATE INDEX discussions_by_activity ON discussions (archived_at IS NOT NULL, updated_at DESC);
CREATE INDEX posts_by_discussion ON posts (discussion_id, created_at);
CREATE INDEX posts_by_parent ON posts (parent_id);
COMMIT;
