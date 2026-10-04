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
   │
Local persistence (from Stage 1)
```

- **nimata-core** holds the domain model and all discussion logic. It has
  no Tauri dependency so that it can be tested directly and reused by other
  clients later.
- **src-tauri** is a thin shell. Commands translate between IPC and core
  calls, and platform concerns (paths, keychain, lifecycle) live here.
- **The UI** talks to Rust only through `src/lib/api.ts`.

Stage 0 serves fixture discussions from `nimata_core::fixtures` through
the real command path. A `nimata://clock-tick` event from Rust keeps
relative timestamps current.

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
2. **Context reference**: other posts the author was asked to consider
   (introduced in Stage 5). Any number.

A model replying to a post receives its ancestry (root to parent) by
default, plus only those context references that were explicitly chosen.
Keeping the two separate means "what this answers" and "what this was shown"
are never confused.
