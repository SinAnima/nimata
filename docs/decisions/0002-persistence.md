# 0002: Rust-owned SQLite is the authoritative store; RxDB is not adopted

Status: accepted (Stage 2)

## Context

The agreed direction was Tauri 2 + RxDB + SQLite, on condition that a spike
confirmed RxDB fits. Nimata's rules that matter here: Rust is the trusted
core, the local database is authoritative, Nimata must work offline and on
phones, and data must stay intelligible without Nimata.

The spike is in `spikes/rxdb/` (RxDB 17.6.0, Node 25, `node:sqlite`). Run it
with `cd spikes/rxdb && npm install && npm run spike`. It is not part of the
app build.

## Findings

1. **RxDB under Tauri and Svelte.** RxDB is a JavaScript database that runs
   in the webview. It has a Tauri adapter (`getSQLiteBasicsTauri`, through
   `@tauri-apps/plugin-sql`), so it can run in Nimata's environment. But the
   data would then be owned by the webview, and the Rust core would have to
   reach discussion data through JavaScript. Every rule Rust enforces today
   (a reply's parent is in the same discussion, creation times never
   change, atomic first posts) would move into JavaScript or be duplicated.
2. **SQLite-backed storage on desktop.** The SQLite storage in the free
   package is a trial (verified in its source and by experiment):
   - writes fail with error `SQL3` after 250 single inserts (a 500
     operation limit), and the document limit is 500;
   - it creates no indexes;
   - its own console message says not to use it in production.

   Production SQLite storage is part of RxDB Premium, a paid licence. The
   free alternative in a webview is IndexedDB (Dexie storage).

3. **iOS and Android.** RxDB itself is plain JavaScript and would run in the
   mobile webviews. Storage would again be either the paid SQLite plugin or
   the webview's IndexedDB, which the operating system manages and which is
   harder to back up, inspect, or restore than a SQLite file. Not tested on
   devices in this spike.
4. **Replication.** RxDB has a well-designed replication protocol with
   checkpoints and many transports (CouchDB, WebRTC, WebSocket, GraphQL,
   and others). In the experiment, two offline devices each added a reply,
   and both replies merged cleanly, as expected for records with
   independently generated IDs.
5. **Schema migration.** Works: migration strategies are JavaScript
   functions run over every document when the schema version increases. There
   is no SQL-level migration, and documents are JSON blobs in one table per
   collection, so the SQLite file is not readable as a normal schema.
6. **Attachments.** Stored inside the storage engine next to the document,
   with a SHA-256 digest. The trial SQLite storage rejects any schema with
   attachments (error `SQL1`).
7. **Conflicts.** The default conflict handler keeps the master state. In
   the experiment both devices renamed the same discussion offline; the
   phone's older rename reached the master first, and the desktop's newer
   rename was dropped on every device without any notice. Custom handlers
   can do better, but the rules are ours to write either way.
8. **Who owns the model.** If RxDB owned the application model, the trusted
   core would be JavaScript in the webview, and production use would depend
   on a paid licence for SQLite storage. As a replication layer only, it
   would require mirroring the Rust-owned data into RxDB collections,
   doubling the storage for no gain until sync exists.

## Decision

- The SQLite database owned by Rust (`rusqlite`) stays the authoritative
  store on every platform. Domain rules are enforced in Rust and, where
  possible, by the schema itself.
- RxDB is not adopted. It does not own the model, and it is not used as a
  local cache.
- Records are designed for replication now: locally generated UUIDv7 IDs,
  immutable creation times, append-only posts, revisions for edits,
  tombstones for deletion, and a `modified_at` time on mutable records.
- Stage 10 chooses a sync transport. RxDB's replication protocol, and a
  CouchDB-compatible transport, remain candidates there as adapters over
  the Rust store; the conflict rules will be Nimata's own and explicit.

## Consequences

- No paid dependency for core functionality, and the database file stays an
  ordinary SQLite schema that can be read with standard tools.
- Backup and restore are SQLite's own online backup, done in Rust.
- Sync needs its own change tracking later, rather than RxDB's built in one.
- If the decision is revisited, the `Repository` trait is the boundary to
  replace; the UI does not know about storage.
