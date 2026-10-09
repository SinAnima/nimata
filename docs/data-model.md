# Data model

The local SQLite database (`nimata.sqlite3` in the app data folder) is the
authoritative record of a user's discussions. It is designed to stay
readable with ordinary tools such as the `sqlite3` command-line shell:

- IDs are UUIDv7 stored as hyphenated text (except imports from ChatGPT,
  whose IDs are UUIDv5 derived from ChatGPT's; see
  [archive-format.md](archive-format.md#chatgpt)). They are generated on the
  device, so records can be created offline and merged later without
  coordination.
- Times are integers: UTC Unix milliseconds.
- A post's original UTC offset is stored next to its instant, in minutes
  east of UTC, so the author's wall-clock time can always be shown exactly.

The schema version is `PRAGMA user_version`; Nimata databases also carry
`PRAGMA application_id = 1313426753` ("NIMA"). Migrations live in
`crates/nimata-core/migrations/` and run in order, each in its own
transaction. Nimata refuses to open a database from a newer version, a
damaged database, or a SQLite file that is not a Nimata database, rather
than risk changing it.

Since version 6 the schema relies on one SQL function Nimata defines on
each connection, `nimata_fold()`, which the search index triggers call.
Reading works with any SQLite tool; writing posts or discussion titles
from outside Nimata fails with "no such function: nimata_fold" rather than
leave the search index out of date.

Migration tests load frozen databases from earlier versions
(`crates/nimata-core/tests/fixtures/`) and check that nothing is lost.

## Tables (schema version 7)

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
| deleted_at  | set when deleted; the row stays as a tombstone      |
| modified_at | last change of any kind, for future sync            |

Renaming or archiving does not change `updated_at`: the discussion list is
ordered by conversation activity, not by bookkeeping.

### posts

| column                 | meaning                                        |
| ---------------------- | ---------------------------------------------- |
| id                     | UUIDv7                                         |
| discussion_id          | the discussion the post belongs to             |
| parent_id              | the post this one replies to, if any           |
| author_id              | participant who wrote it                       |
| body                   | text as written                                |
| created_at             | UTC instant, never rewritten                   |
| tz_offset_minutes      | author's UTC offset when writing               |
| edited_at              | when the text was last changed                 |
| deleted_at             | set when deleted; the row stays as a tombstone |
| modified_at            | last change of any kind, for future sync       |
| status                 | `complete`; model replies add other states     |
| provider_metadata_json | provider details for model posts (Stage 3)     |

Rules enforced on every write. The ones marked (schema) are triggers in the
database itself, so they hold even for changes made outside Nimata's code:

- A post's ID, discussion, parent, author, creation time, and UTC offset
  never change after it is written (schema). The same applies to a
  discussion's ID and creation time.
- A deleted post cannot be replied to or edited.
- Only a post's author can edit or delete it.

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

### post_revisions

Earlier versions of edited posts. Editing a post moves its current text
here before replacing it, so history is never silently rewritten.

| column      | meaning                               |
| ----------- | ------------------------------------- |
| id          | UUIDv7                                |
| post_id     | the edited post                       |
| body        | the text before the edit              |
| written_at  | when that text became the post's body |
| replaced_at | when the edit replaced it             |

Revisions cannot be modified (schema). They are erased together with their
post when the post is deleted.

### attachments

Metadata for files attached to posts; the bytes are in the blob store. The
table exists from schema version 2 and is used from version 7 (see
[below](#attachments-schema-version-7)).

| column       | meaning                                      |
| ------------ | -------------------------------------------- |
| id           | UUIDv7                                       |
| post_id      | the post the file is attached to             |
| filename     | original file name                           |
| media_type   | e.g. `text/markdown`                         |
| size         | bytes                                        |
| content_hash | `sha256:<hex>`; identical files share a hash |
| kind         | `text`, `image`, `pdf`, or `other` (v7)      |
| position     | order among the post's files (v7)            |
| created_at   | when it was attached                         |
| local_uri    | unused                                       |
| deleted_at   | unused: deleting a post deletes its rows     |

## Deletion

Deleting never removes rows; it leaves tombstones, so a future sync can tell
other devices about the deletion instead of the record reappearing.

- **A post**: its text and revisions are erased and `deleted_at` is set. It
  keeps its place in the thread, so replies to it still make sense; the UI
  shows "This post was deleted."
- **A discussion**: its title, every post's text, the revisions, and the
  draft are erased, and `deleted_at` is set. It no longer appears anywhere.

## Backup and restore

A backup is a single SQLite file written with SQLite's online backup, so it
is consistent even while Nimata is running. It is verified before it is
moved into place under the chosen name.

Restoring checks that the file is an undamaged Nimata database from this or
an earlier version, keeps a safety copy of the current data in the
`safety-copies` folder inside the data folder, replaces the data, and
migrates it if it came from an older version. The search indexes are then rebuilt
from the restored posts.

If the database cannot be opened at startup (damaged, or not a Nimata
database), Nimata says why and offers to restore a backup. The unusable file
is kept beside the new one with a `.damaged-<time>` suffix, never deleted.

## Models (schema version 3)

### providers

Non-secret settings for each provider connection. API keys are never stored
here (see [privacy.md](privacy.md#api-keys)).

| column                  | meaning                                            |
| ----------------------- | -------------------------------------------------- |
| id                      | UUIDv7; also names the key in the credential store |
| kind                    | `openai`, `anthropic`, or `openai_compatible`      |
| display_name            | e.g. "OpenAI"                                      |
| base_url                | endpoint, or null for the provider's standard one  |
| created_at, modified_at | bookkeeping                                        |

Model participants are rows in `participants` with `kind = 'model'`, plus
`provider_id` and `enabled`. One participant per provider and model.

### generations

One row per request to a model. The reply itself is an ordinary post.

| column                  | meaning                                                             |
| ----------------------- | ------------------------------------------------------------------- |
| id                      | UUIDv7                                                              |
| post_id                 | the reply post                                                      |
| participant_id          | the model asked                                                     |
| status                  | `queued`, `sending`, `streaming`, `complete`, `failed`, `cancelled` |
| error                   | message for people, when failed                                     |
| context_post_ids        | JSON array: exactly which posts were sent, in order                 |
| started_at, finished_at | when it ran                                                         |

A reply post's `status` follows its request (`streaming` while it runs),
and `provider_metadata_json` holds the provider, exact model version,
provider response and request IDs, and token counts once it finishes.
Requests still running at startup are marked failed with "Nimata was closed
before this reply was finished", keeping the text received.

Model replies cannot be edited. The person whose discussion it is can
delete them, once they are no longer streaming.

## Aliases and settings (schema version 4)

`participants.aliases` holds a model's own aliases as a JSON array of
lowercase strings, e.g. `["review", "r"]`. Each names exactly one model:
an alias cannot equal another model's alias or automatic alias (its name in
lowercase letters and digits).

`app_settings` is a key-value table for small preferences. `default_model`
holds the participant ID of the model asked when neither a mention nor the
thread decides.

## Context references (schema version 5)

A post has one reply edge (`posts.parent_id`, where it sits in the thread)
and any number of context references: other posts in the same discussion
its author chose to take into account.

### context_refs

| column      | meaning                         |
| ----------- | ------------------------------- |
| post_id     | the post that chose the context |
| ref_post_id | the post chosen                 |
| position    | order chosen                    |

References are fixed once the post exists (schema). They must point to an
undeleted post in the same discussion; a reference to the post being
replied to is dropped as redundant.

`drafts.context_ids` keeps the context chosen for an unsent post (JSON
array). `generations.sent_json` records exactly what a request sent: the
model, instructions, each message with its source (`thread` or `context`)
and post IDs, what was left out and why (`deleted`, `unfinished`,
`trimmed`), and the size estimate and budget.

## Search (schema version 6)

`posts_fts` and `discussions_fts` are FTS5 full-text indexes over post
bodies and discussion titles. They hold text folded by `nimata_fold()`
(lowercase, accents removed in any script) and store no text of their own;
rows are matched to `posts` and `discussions` by `rowid`, and triggers keep
them current on every insert, edit, streamed update, and delete. Deleted
posts have empty bodies, so they leave the index. `posts_fts` also keeps
prefix indexes for two and three letters.

### recent_searches

| column       | meaning                     |
| ------------ | --------------------------- |
| query        | the search text, as typed   |
| last_used_at | when it was last used (UTC) |

Only searches whose results were opened are remembered, the ten most
recent. They stay on this device and are not part of discussion exports.

See [search.md](search.md) for the query syntax.

## Attachments (schema version 7)

Attached files' bytes live in the blob store, `blobs/sha256/ab/<hash>` in
the data folder, named by content hash; see [attachments.md](attachments.md).
Version 7 starts using the `attachments` table:

- `kind` records what Nimata can do with the file, decided from its
  contents when it was attached; `position` keeps the order.
- Attachment rows never change once posted (a trigger enforces this).
  Deleting a post deletes its rows, and the bytes are deleted once no
  attachment or draft refers to their hash.
- `drafts.attachments` (JSON array) holds files added to an unsent post:
  `filename`, `mediaType`, `size`, `contentHash`, `kind`.

A backup written by Nimata 7 or later has one more table,
`backup_files (content_hash, bytes)`, holding a copy of every attached file.
Restoring puts those files back in the blob store and removes the table.
