-- Nimata schema version 1.
-- Times are UTC Unix milliseconds. Offsets are minutes east of UTC.

CREATE TABLE participants (
    id                 TEXT PRIMARY KEY,
    kind               TEXT NOT NULL CHECK (kind IN ('human', 'model', 'agent')),
    display_name       TEXT NOT NULL,
    provider           TEXT,
    model              TEXT,
    configuration_json TEXT,
    is_local_user      INTEGER NOT NULL DEFAULT 0
);

-- At most one participant represents the person using this device.
CREATE UNIQUE INDEX participants_local_user ON participants (is_local_user) WHERE is_local_user = 1;

CREATE TABLE discussions (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    archived_at INTEGER,
    pinned_at   INTEGER
);

CREATE INDEX discussions_by_activity ON discussions (archived_at IS NOT NULL, updated_at DESC);

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

CREATE INDEX posts_by_discussion ON posts (discussion_id, created_at);
CREATE INDEX posts_by_parent ON posts (parent_id);

-- One unsent draft per discussion, including the post it would reply to.
CREATE TABLE drafts (
    discussion_id TEXT PRIMARY KEY REFERENCES discussions (id),
    parent_id     TEXT REFERENCES posts (id),
    body          TEXT NOT NULL,
    updated_at    INTEGER NOT NULL
);
