-- Nimata schema version 6: full-text search over posts and discussion
-- titles, and recent searches.
--
-- The indexes hold text folded by nimata_fold(), a function Nimata defines
-- on every connection: lowercase, without accents in any script, so
-- "νηματα" finds "Νήματα" and "cafe" finds "Café". (SQLite's own
-- diacritic removal only covers Latin letters.) They store no text of their
-- own (contentless); snippets are made from the posts themselves.
--
-- prefix = '2 3' keeps searches for short word beginnings fast while
-- typing. Rows are matched by rowid. Nimata never runs VACUUM, which could
-- renumber rowids, and rebuilds the indexes after restoring a backup.

CREATE VIRTUAL TABLE posts_fts USING fts5(
    body,
    content = '',
    contentless_delete = 1,
    prefix = '2 3',
    tokenize = 'unicode61 remove_diacritics 2'
);

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

CREATE VIRTUAL TABLE discussions_fts USING fts5(
    title,
    content = '',
    contentless_delete = 1,
    tokenize = 'unicode61 remove_diacritics 2'
);

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

-- Index what already exists.
INSERT INTO posts_fts (rowid, body) SELECT rowid, nimata_fold(body) FROM posts;
INSERT INTO discussions_fts (rowid, title) SELECT rowid, nimata_fold(title) FROM discussions;

-- Speeds up filtering by author and date.
CREATE INDEX posts_by_author ON posts (author_id, created_at);
CREATE INDEX posts_by_time ON posts (created_at);

CREATE TABLE recent_searches (
    query        TEXT PRIMARY KEY,
    last_used_at INTEGER NOT NULL
);
