# Data model

The local SQLite database (`nimata.sqlite3` in the app data folder) is the
authoritative record of a user's discussions. It is designed to stay
readable with ordinary tools such as the `sqlite3` command-line shell:

- IDs are UUIDv7 stored as hyphenated text. They are generated on the
  device, so records can be created offline and merged later without
  coordination.
- Times are integers: UTC Unix milliseconds.
- A post's original UTC offset is stored next to its instant, in minutes
  east of UTC, so the author's wall-clock time can always be shown exactly.

The schema version is `PRAGMA user_version`. Migrations live in
`crates/nimata-core/migrations/` and run in order, each in its own
transaction. Nimata refuses to open a database from a newer version rather
than risk damaging it.

## Tables (schema version 1)

### participants

| column             | meaning                                         |
| ------------------ | ----------------------------------------------- |
| id                 | UUIDv7                                          |
| kind               | `human`, `model`, or `agent`                    |
| display_name       | name shown on posts                             |
| provider, model    | for model participants (from Stage 3)           |
| configuration_json | participant-specific settings (from Stage 3)    |
| is_local_user      | 1 for the person using this device; at most one |

### discussions

| column      | meaning                                             |
| ----------- | --------------------------------------------------- |
| id          | UUIDv7                                              |
| title       | 1 to 200 characters                                 |
| created_at  | when the discussion was started                     |
| updated_at  | time of the latest post; orders the discussion list |
| archived_at | set while archived, null otherwise                  |
| pinned_at   | reserved for pinning                                |

Renaming or archiving does not change `updated_at`: the discussion list is
ordered by conversation activity, not by bookkeeping.

### posts

| column                 | meaning                                    |
| ---------------------- | ------------------------------------------ |
| id                     | UUIDv7                                     |
| discussion_id          | the discussion the post belongs to         |
| parent_id              | the post this one replies to, if any       |
| author_id              | participant who wrote it                   |
| body                   | text as written                            |
| created_at             | UTC instant, never rewritten               |
| tz_offset_minutes      | author's UTC offset when writing           |
| edited_at              | reserved for edits (Stage 2)               |
| status                 | `complete`; model replies add other states |
| provider_metadata_json | provider details for model posts (Stage 3) |

Rules enforced on every write:

- A reply's parent must be a post in the same discussion.
- A post must contain text. Surrounding blank lines are removed. Other
  whitespace is kept as written.
- Starting a discussion inserts the discussion and its first post in one
  transaction, so there are no empty discussions.

Posts are ordered by `created_at` only. If the device clock was wrong when a
post was written, the stored time is still what the device reported. Nimata
does not adjust it.

### drafts

One unsent post per discussion: `body`, the `parent_id` it would reply to,
and `updated_at`. A draft with no text and no reply target is deleted.
Posting deletes the draft in the same transaction.
