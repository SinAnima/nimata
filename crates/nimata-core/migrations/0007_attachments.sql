-- Nimata schema version 7: attachments are stored and posted.
--
-- The attachments table has existed since version 2 but was never written.
-- Bytes live in the blob store (blobs/sha256/ab/<hash> in the data folder);
-- rows hold metadata. An attachment never changes once posted; deleting its
-- post deletes the row, and the bytes once nothing else refers to them.

ALTER TABLE attachments ADD COLUMN kind TEXT NOT NULL DEFAULT 'other'
    CHECK (kind IN ('text', 'image', 'pdf', 'other'));
ALTER TABLE attachments ADD COLUMN position INTEGER NOT NULL DEFAULT 0;

CREATE TRIGGER attachments_are_immutable
BEFORE UPDATE ON attachments
BEGIN
    SELECT RAISE(ABORT, 'attachments cannot change once posted');
END;

-- Files added to an unsent post (JSON array of staged attachments).
ALTER TABLE drafts ADD COLUMN attachments TEXT NOT NULL DEFAULT '[]';
