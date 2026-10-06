# Architecture

## Ownership

A Nimata discussion belongs to the user and lives in a local database on
their device. Model providers are participants that are asked to write
posts; they never own or identify a discussion. Provider identifiers are
kept only as metadata on the posts they produced.

## Layers

```
Svelte UI (src/)
   │  Tauri commands (request/response), events (Rust to UI)
Tauri shell (src-tauri/)
   │  plain Rust calls
Nimata core (crates/nimata-core/)
   │  Repository trait
SQLite database in the app data folder (nimata.sqlite3)
```

- **nimata-core** holds the domain model and all discussion logic. It has
  no Tauri dependency so that it can be tested directly and reused by other
  clients later.
- **src-tauri** is a thin shell. Commands translate between IPC and core
  calls, and platform concerns (paths, keychain, lifecycle) live here.
- **The UI** talks to Rust only through `src/lib/api.ts`.

- **Persistence** sits behind the `Repository` trait in
  `nimata-core/src/repository.rs`. `SqliteRepository` is the only
  implementation. Rust owns the SQLite database on every platform; RxDB was
  evaluated and not adopted (see
  [ADR 0002](decisions/0002-persistence.md)). The database is opened once
  at startup and held in Tauri state behind a mutex. If it cannot be
  opened, the UI shows the reason and offers to restore a backup instead of
  the app crashing, and the unusable file is never modified or deleted.

Model replies are run by `src-tauri/src/generation.rs`: it builds the
request from the thread (`nimata_core::context`), calls the provider
adapter (`nimata_core::providers`), streams text into the reply post, and
announces progress to the UI as `nimata://post-delta` and
`nimata://post-updated` events. See [providers.md](providers.md) and
[ADR 0003](decisions/0003-model-providers.md).

Rust reads the clock for every write. A post's instant and UTC offset come
from the same read of the device clock, so the UI cannot supply or alter
them. A `nimata://clock-tick` event from Rust keeps relative timestamps in
the UI current.

Drafts are saved shortly after typing pauses, and again when the window is
hidden or closed. Posting removes the draft in the same transaction that
inserts the post.

See [data-model.md](data-model.md) for the schema, deletion, backup, and
restore, and [archive-format.md](archive-format.md) for the canonical JSON
export.

## Domain model

- **Discussion**: a titled collection of posts.
- **Post**: written by one participant, optionally replying to one earlier
  post in the same discussion (`parent_id`).
- **Participant**: a human, model, or later an agent.

IDs are UUIDv7, generated locally so a device can create records offline.
Times are UTC Unix milliseconds plus the author's UTC offset in minutes.
Ordering uses the instant only, never IDs or provider data.

## Why a chronological stream

Posts are stored as a tree, because a post can reply to any earlier post.
Rendering that tree as nested, increasingly indented comments works for
short threads but breaks down for long discussions: the reading order
jumps around in time, and deep branches get squeezed into narrow columns,
which is unusable on a phone.

Nimata shows posts in the order they were written, as a correspondence
client does. Each reply carries a "Replying to" reference. When the parent
is not the post directly above, the reference quotes the parent's first
line, and activating it scrolls to the parent and highlights it. The tree
stays fully navigable without sacrificing chronological reading.

## Reply relationship and context relationship

Two different relationships connect posts:

1. **Reply**: the post this one answers (`parent_id`). Exactly zero or one.
2. **Context reference**: other posts the author chose to take into
   account, with Include. Any number, from any branch of the same
   discussion.

A model replying to a post receives its ancestry (root to parent), plus the
context references made anywhere in that chain, sent in a labelled message
so the model can tell them apart (`nimata_core::context`). Exactly what was
sent is recorded with each request.
Keeping the two separate means "what this answers" and "what this was shown"
are never confused.
