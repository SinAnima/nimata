-- Nimata schema version 5: context references, and an exact record of what
-- each model was sent.

-- Posts an author asked to be taken into account, besides the post being
-- replied to. A reply edge says where a post sits in the thread; a context
-- reference only adds material. Both are fixed once the post exists.
CREATE TABLE context_refs (
    post_id     TEXT NOT NULL REFERENCES posts (id),
    ref_post_id TEXT NOT NULL REFERENCES posts (id),
    position    INTEGER NOT NULL,
    PRIMARY KEY (post_id, ref_post_id)
);

CREATE INDEX context_refs_by_ref ON context_refs (ref_post_id);

CREATE TRIGGER context_refs_are_immutable
BEFORE UPDATE ON context_refs
BEGIN
    SELECT RAISE(ABORT, 'context references cannot change once a post exists');
END;

-- Context chosen for an unsent post, kept with its draft.
ALTER TABLE drafts ADD COLUMN context_ids TEXT NOT NULL DEFAULT '[]';

-- Exactly what was sent with a request: instructions, each message with
-- where it came from, what was left out and why, and the size estimate.
ALTER TABLE generations ADD COLUMN sent_json TEXT;
