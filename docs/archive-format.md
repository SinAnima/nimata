# Export and archive formats

Nimata can export one discussion as JSON (`nimata/1`) or Markdown, and
everything as an archive (`nimata-archive/1`, a zip of `nimata/1` files).
Archives and `nimata/1` files can be imported again, on this device or
another; ChatGPT's data export can be imported too.

## One discussion (`nimata/1`)

Nimata can export a discussion as JSON in a documented format that does not
depend on its database layout. The file is meant to stay intelligible
without Nimata: a person can read it, and any program can parse it.

Export a discussion from the discussion's More menu: **Export as JSON…**.
Files are named `<title>.nimata.json`.

**Export as Markdown…** in the same menu writes the same content for
reading: the posts in the order they were written, each with its author
(and model version for model replies), its exact time in the author's UTC
offset, and links to the post it replies to and any posts it took as
context. Markdown is for reading and sharing; it cannot be imported.

### Example

```json
{
  "format": "nimata/1",
  "exportedAt": "2026-10-05T14:03:22.000Z",
  "discussion": {
    "id": "01a10970-0000-7000-8000-000000000001",
    "title": "Should Nimata use CouchDB?",
    "createdAt": "2026-10-04T16:41:07.123Z",
    "archivedAt": null
  },
  "participants": [
    {
      "id": "01a10969-fa3e-754f-84da-3849ad072024",
      "kind": "human",
      "displayName": "Thanos",
      "localUser": true
    }
  ],
  "posts": [
    {
      "id": "01a10971-0000-7000-8000-000000000001",
      "parentId": null,
      "authorId": "01a10969-fa3e-754f-84da-3849ad072024",
      "createdAt": "2026-10-04T12:41:07.123-04:00",
      "status": "complete",
      "body": "Should Nimata use CouchDB?"
    },
    {
      "id": "01a10971-0000-7000-8000-000000000002",
      "parentId": "01a10971-0000-7000-8000-000000000001",
      "authorId": "01a10969-fa3e-754f-84da-3849ad072024",
      "createdAt": "2026-10-04T19:43:07.123+03:00",
      "status": "complete",
      "body": "The attraction is replication.",
      "editedAt": "2026-10-04T16:46:00.000Z",
      "revisions": [
        {
          "body": "The attraction is replicaton.",
          "writtenAt": "2026-10-04T16:43:07.123Z",
          "replacedAt": "2026-10-04T16:46:00.000Z"
        }
      ]
    }
  ]
}
```

### Fields

#### Top level

| Field          | Meaning                                     |
| -------------- | ------------------------------------------- |
| `format`       | Always `"nimata/1"` for this version.       |
| `exportedAt`   | When the file was written, UTC.             |
| `discussion`   | The discussion itself.                      |
| `participants` | Everyone who wrote a post in it.            |
| `posts`        | Every post, in the order they were written. |

#### `discussion`

`id` (UUID), `title`, `createdAt` (UTC), and `archivedAt` (UTC, or `null`).

#### `participants[]`

`id`, `kind` (`human`, `model`, or `agent`), `displayName`, and for models
`provider` and `model`. `localUser` is `true` for the person who exported
the file. Fields without a value are omitted.

#### `posts[]`

| Field              | Meaning                                                                                                                                                                                                                            |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `id`               | UUID of the post.                                                                                                                                                                                                                  |
| `parentId`         | The post this one replies to, or `null` for a new thread.                                                                                                                                                                          |
| `authorId`         | A participant `id`.                                                                                                                                                                                                                |
| `createdAt`        | When it was written, in the author's own UTC offset.                                                                                                                                                                               |
| `status`           | `complete`; model replies can also be `failed` or `cancelled`.                                                                                                                                                                     |
| `body`             | The current text. Empty for a deleted post.                                                                                                                                                                                        |
| `editedAt`         | When the text was last changed, UTC. Omitted if never edited.                                                                                                                                                                      |
| `deletedAt`        | When the post was deleted, UTC. Omitted unless deleted.                                                                                                                                                                            |
| `revisions`        | Earlier versions, oldest first. Omitted when there are none.                                                                                                                                                                       |
| `providerMetadata` | For model replies: `provider`, `model` (exact version), `responseId`, `requestId`, `inputTokens`, `outputTokens`, `incompleteReason`. Omitted otherwise.                                                                           |
| `attachments`      | Files attached to the post: `id`, `filename`, `mediaType`, `size`, `contentHash` (`sha256:<hex>`), `kind` (`text`, `image`, `pdf`, `other`), `createdAt`. Omitted when empty. The bytes are not in this file; archives carry them. |
| `contextIds`       | Other posts the author chose as context, besides the one replied to. Omitted when empty.                                                                                                                                           |

A deleted post stays in the file with an empty body so that replies to it
still have a parent. Its revisions are erased when it is deleted.

#### `revisions[]`

Each is the text the post had from `writtenAt` until an edit replaced it at
`replacedAt` (both UTC).

### Times

All times are RFC 3339 strings with millisecond precision.

- `createdAt` on a post uses the offset the author's device had when the
  post was written, for example `2026-10-04T12:41:07.123-04:00`. This keeps
  both the exact instant and the author's wall-clock time.
- Every other time is in UTC (`Z`).

Order posts by the instant, not by the text of the time string: two posts
written in different offsets do not sort correctly as strings.

### Guarantees

- Every `parentId` refers to a post in the same file.
- Every `authorId` refers to a participant in the same file.
- Every ID in `contextIds` refers to a post in the same file.
- IDs are Nimata's own. Provider identifiers, when they exist (Stage 3 on),
  appear only as metadata on model posts, never as IDs.

Readers should reject files whose `format` they do not recognise.

## Everything (`nimata-archive/1`)

Settings, Data, **Export all discussions…** writes a zip named
`nimata-archive-<date>.zip`:

```
manifest.json
discussions/<discussion id>.json    one nimata/1 file per discussion
attachments/<hex of the sha256>     the bytes of every attached file
```

`manifest.json`:

```json
{
  "format": "nimata-archive/1",
  "exportedAt": "2026-10-07T14:03:22.000Z",
  "discussions": [
    {
      "id": "01a10970-0000-7000-8000-000000000001",
      "title": "Should Nimata use CouchDB?",
      "path": "discussions/01a10970-0000-7000-8000-000000000001.json",
      "posts": 3
    }
  ],
  "attachments": [
    "sha256:9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"
  ]
}
```

`attachments` lists the files the archive carries. Each is stored once,
however many posts it is attached to; a file not on the exporting device is
listed on its post but not carried.

Active and archived discussions are included; deleted ones are not.
Drafts, providers, model settings, recent searches, and API keys are not
part of an archive (a [backup](data-model.md#backup-and-restore) holds all
but the keys).

## Importing

Settings, Data, **Import…** accepts:

| File                            | Recognised by                           |
| ------------------------------- | --------------------------------------- |
| A Nimata archive (`.zip`)       | `manifest.json` with `nimata-archive/1` |
| One Nimata discussion (`.json`) | `"format": "nimata/1"`                  |
| A ChatGPT export (`.zip`)       | `conversations.json` inside it          |
| ChatGPT's `conversations.json`  | a JSON array of conversations           |

Rules:

- **Matched by ID.** A discussion or post whose ID is already here is not
  added again, so importing the same file twice changes nothing, and
  importing a newer export adds only the posts this device lacks. Posts
  and titles already here are never overwritten by an import.
- **Your posts stay yours.** Posts by the participant marked `localUser`
  become the importing person's own, so they can be edited and deleted.
  Other people and models are added as participants under their own IDs.
  Imported models are authors only; to ask one, add it in Settings,
  Models.
- **Deleted stays deleted.** A discussion deleted on this device is not
  brought back by importing an older export of it.
- **Each discussion is all or nothing.** If one cannot be imported (for
  example, it claims a post that belongs to another discussion), nothing
  of it is added, the others still are, and the reason is shown.
- A reply that was still being written when exported is imported as
  failed, keeping its text.
- **Files are checked.** Every file in an archive must match its hash, or
  the import stops before changing anything. A file listed on a post but
  not carried is left out and counted in the summary.

### ChatGPT

ChatGPT's data export (Settings, Data controls, Export data) is a zip with
`conversations.json` inside. Each conversation becomes a discussion:

- Your messages are yours; answers are by a model participant named after
  the model ChatGPT recorded, such as "ChatGPT (gpt-4o)".
- Edited prompts and regenerated answers are separate branches in ChatGPT;
  every branch is kept, as replies to the same post.
- System messages, hidden messages, tool calls and their output, and
  reasoning summaries are left out. Images are noted as
  "[Image not included in the import]".
- Nimata IDs are name-based UUIDs (version 5) derived from ChatGPT's
  conversation and message IDs, so importing a later export adds only new
  messages. Each answer keeps ChatGPT's message ID as `responseId` in its
  provider metadata.
- The export has no time zones, so imported times are in UTC.
