# Discussion archive format (`nimata/1`)

Nimata can export a discussion as JSON in a documented format that does not
depend on its database layout. The file is meant to stay intelligible
without Nimata: a person can read it, and any program can parse it.

Export a discussion from the discussion's More menu: **Export as JSON…**.
Files are named `<title>.nimata.json`.

## Example

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
      "displayName": "Thanos"
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

## Fields

### Top level

| Field          | Meaning                                     |
| -------------- | ------------------------------------------- |
| `format`       | Always `"nimata/1"` for this version.       |
| `exportedAt`   | When the file was written, UTC.             |
| `discussion`   | The discussion itself.                      |
| `participants` | Everyone who wrote a post in it.            |
| `posts`        | Every post, in the order they were written. |

### `discussion`

`id` (UUID), `title`, `createdAt` (UTC), and `archivedAt` (UTC, or `null`).

### `participants[]`

`id`, `kind` (`human`, `model`, or `agent`), `displayName`, and for models
`provider` and `model`. Fields without a value are omitted.

### `posts[]`

| Field              | Meaning                                                                                                                                                  |
| ------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `id`               | UUID of the post.                                                                                                                                        |
| `parentId`         | The post this one replies to, or `null` for a new thread.                                                                                                |
| `authorId`         | A participant `id`.                                                                                                                                      |
| `createdAt`        | When it was written, in the author's own UTC offset.                                                                                                     |
| `status`           | `complete`; model replies can also be `failed` or `cancelled`.                                                                                           |
| `body`             | The current text. Empty for a deleted post.                                                                                                              |
| `editedAt`         | When the text was last changed, UTC. Omitted if never edited.                                                                                            |
| `deletedAt`        | When the post was deleted, UTC. Omitted unless deleted.                                                                                                  |
| `revisions`        | Earlier versions, oldest first. Omitted when there are none.                                                                                             |
| `providerMetadata` | For model replies: `provider`, `model` (exact version), `responseId`, `requestId`, `inputTokens`, `outputTokens`, `incompleteReason`. Omitted otherwise. |
| `contextIds`       | Other posts the author chose as context, besides the one replied to. Omitted when empty.                                                                 |

A deleted post stays in the file with an empty body so that replies to it
still have a parent. Its revisions are erased when it is deleted.

### `revisions[]`

Each is the text the post had from `writtenAt` until an edit replaced it at
`replacedAt` (both UTC).

## Times

All times are RFC 3339 strings with millisecond precision.

- `createdAt` on a post uses the offset the author's device had when the
  post was written, for example `2026-10-04T12:41:07.123-04:00`. This keeps
  both the exact instant and the author's wall-clock time.
- Every other time is in UTC (`Z`).

Order posts by the instant, not by the text of the time string: two posts
written in different offsets do not sort correctly as strings.

## Guarantees

- Every `parentId` refers to a post in the same file.
- Every `authorId` refers to a participant in the same file.
- Every ID in `contextIds` refers to a post in the same file.
- IDs are Nimata's own. Provider identifiers, when they exist (Stage 3 on),
  appear only as metadata on model posts, never as IDs.

Importing archives arrives in Stage 7. Readers should reject files whose
`format` they do not recognise.
