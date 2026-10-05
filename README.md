# Nimata

**Nimata is a local-first discussion client for conversations with humans
and AI models.**

Instead of keeping your GPT conversations in one application, Claude
discussions in another, and Gemini research somewhere else, Nimata stores
discussions locally and treats models as participants.

Reply to GPT. Ask Claude to challenge GPT. Ask Qwen to respond to Claude.
Return six months later and search the whole discussion without needing to
remember which provider you used.

Your discussions belong to you.

The name comes from the Greek νήματα, "threads".

## Status

Early development. Nimata works as a private threaded notebook: start
discussions, reply to any post, rename and archive discussions, and keep
unsent drafts. Everything is stored in a local SQLite database. Model
participants arrive in a later stage.

Keyboard shortcuts on desktop:

| Keys      | Action                        |
| --------- | ----------------------------- |
| Cmd+N     | New discussion                |
| Cmd+Enter | Post, or start the discussion |
| J / K     | Next / previous post          |
| R         | Reply to the focused post     |
| Esc       | Cancel the reply target       |

## Principles

- The local database is the authoritative record. No account and no Nimata
  server are required.
- Models are participants. Provider conversation IDs are kept only as
  secondary metadata, never as Nimata's identity for a discussion.
- Every post records an exact UTC instant and the author's UTC offset.
- Discussions are stored as threads but read as a chronological stream. See
  [docs/architecture.md](docs/architecture.md#why-a-chronological-stream).

## Development

See [docs/development.md](docs/development.md) for setup, commands, and
checks.
