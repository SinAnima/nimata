-- Nimata schema version 3: model providers, model participants, and the
-- durable record of each request to a model.

-- Non-secret provider settings. API keys are never stored here; they live in
-- the operating system's credential store, keyed by provider ID.
CREATE TABLE providers (
    id           TEXT PRIMARY KEY,
    kind         TEXT NOT NULL CHECK (kind IN ('openai', 'anthropic', 'openai_compatible')),
    display_name TEXT NOT NULL,
    base_url     TEXT,
    created_at   INTEGER NOT NULL,
    modified_at  INTEGER NOT NULL
);

-- A model participant belongs to a provider; disabling one hides it from
-- "Ask" without touching the posts it wrote.
ALTER TABLE participants ADD COLUMN provider_id TEXT REFERENCES providers (id);
ALTER TABLE participants ADD COLUMN enabled INTEGER NOT NULL DEFAULT 1;

CREATE UNIQUE INDEX participants_one_per_model
    ON participants (provider_id, model)
    WHERE provider_id IS NOT NULL;

-- One row per request to a model. The reply itself is an ordinary post; this
-- records how it came about and exactly which posts were sent as context.
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

CREATE INDEX generations_by_post ON generations (post_id);
CREATE INDEX generations_unfinished
    ON generations (status)
    WHERE status IN ('queued', 'sending', 'streaming');
