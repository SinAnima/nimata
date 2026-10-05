-- Nimata schema version 4: short names for models, and app settings.

-- Aliases a model can be mentioned by, e.g. ["review", "r"], as a JSON
-- array. Each alias names exactly one model.
ALTER TABLE participants ADD COLUMN aliases TEXT NOT NULL DEFAULT '[]';

-- Small preferences, such as which model answers when none is implied.
CREATE TABLE app_settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
