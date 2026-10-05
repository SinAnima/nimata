// RxDB evaluation for Nimata (docs/decisions/0002-persistence.md).
// Run: cd spikes/rxdb && npm install && npm run spike
// Each experiment prints what it observed. Nothing here ships in Nimata.

import { DatabaseSync } from "node:sqlite";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { addRxPlugin, createRxDatabase, lastOfArray } from "rxdb";
import { getRxStorageMemory } from "rxdb/plugins/storage-memory";
import {
  getRxStorageSQLiteTrial,
  getSQLiteBasicsNodeNative,
} from "rxdb/plugins/storage-sqlite";
import { RxDBMigrationSchemaPlugin } from "rxdb/plugins/migration-schema";
import { RxDBAttachmentsPlugin } from "rxdb/plugins/attachments";
import { replicateRxCollection } from "rxdb/plugins/replication";

addRxPlugin(RxDBMigrationSchemaPlugin);
addRxPlugin(RxDBAttachmentsPlugin);

const dir = mkdtempSync(join(tmpdir(), "nimata-rxdb-"));
const sqliteStorage = () =>
  getRxStorageSQLiteTrial({
    sqliteBasics: getSQLiteBasicsNodeNative(DatabaseSync),
    databaseNamePrefix: dir + "/",
  });

const postSchema = (version, { withAttachments = false } = {}) => ({
  version,
  primaryKey: "id",
  type: "object",
  properties: {
    id: { type: "string", maxLength: 40 },
    discussionId: { type: "string", maxLength: 40 },
    parentId: { type: ["string", "null"] },
    body: { type: "string" },
    createdAt: { type: "number" },
    ...(version >= 1 ? { tzOffsetMinutes: { type: "number" } } : {}),
  },
  required: ["id", "discussionId", "body", "createdAt"],
  ...(withAttachments ? { attachments: {} } : {}),
});

function report(name, observations) {
  console.log(`\n## ${name}`);
  for (const line of observations) console.log(`- ${line}`);
}

// 1. The free SQLite storage is a trial with hard limits.
async function trialLimits() {
  const db = await createRxDatabase({
    name: "limits",
    storage: sqliteStorage(),
  });
  await db.addCollections({ posts: { schema: postSchema(0) } });
  let written = 0;
  let error = null;
  try {
    for (; written < 1000; written++) {
      await db.posts.insert({
        id: `p${written}`,
        discussionId: "d",
        parentId: null,
        body: "x",
        createdAt: written,
      });
    }
  } catch (e) {
    error = e.code ?? e.message;
  }
  await db.close().catch(() => {});
  report("SQLite trial storage limits", [
    `inserted ${written} posts one at a time before failing`,
    `error: ${error ?? "none"}`,
  ]);
}

// 2. Schema migration runs JavaScript strategies over every document.
async function migration() {
  const name = "migration";
  // Reopening must reuse the same storage object.
  const storage = sqliteStorage();
  const v0 = await createRxDatabase({ name, storage });
  await v0.addCollections({ posts: { schema: postSchema(0) } });
  await v0.posts.bulkInsert(
    [1, 2, 3].map((n) => ({
      id: `p${n}`,
      discussionId: "d",
      parentId: null,
      body: `post ${n}`,
      createdAt: n,
    })),
  );
  await v0.close();

  const v1 = await createRxDatabase({ name, storage });
  await v1.addCollections({
    posts: {
      schema: postSchema(1),
      migrationStrategies: { 1: (doc) => ({ ...doc, tzOffsetMinutes: 0 }) },
    },
  });
  const docs = await v1.posts.find().exec();
  await v1.close();
  report("Schema migration", [
    `${docs.length} documents migrated from version 0 to 1`,
    `example: ${JSON.stringify(docs[0].toJSON())}`,
    "strategies are JavaScript functions keyed by version; there is no SQL-level migration",
  ]);
}

// 3. Attachments are stored with the document and get a content digest.
async function attachments() {
  const db = await createRxDatabase({
    name: "attachments",
    storage: getRxStorageMemory(),
  });
  await db.addCollections({
    posts: { schema: postSchema(0, { withAttachments: true }) },
  });
  const post = await db.posts.insert({
    id: "p1",
    discussionId: "d",
    parentId: null,
    body: "with file",
    createdAt: 1,
  });
  const attachment = await post.putAttachment({
    id: "notes.md",
    data: new Blob(["# Notes"], { type: "text/markdown" }),
    type: "text/markdown",
  });
  await db.close();

  let sqliteError = "none";
  try {
    const sqliteDb = await createRxDatabase({
      name: "attachments",
      storage: sqliteStorage(),
    });
    await sqliteDb.addCollections({
      posts: { schema: postSchema(0, { withAttachments: true }) },
    });
  } catch (e) {
    sqliteError = e.code ?? e.message;
  }
  report("Attachments", [
    `memory storage: stored ${attachment.length} bytes with digest ${attachment.digest}`,
    "attachment data lives inside the storage engine, next to the document",
    `SQLite trial storage with an attachments schema: error ${sqliteError}`,
  ]);
}

// 4. Offline replication: two devices, one shared master.
async function replicationConflicts() {
  // The shared master: key -> { doc, seq }. seq orders changes for pulls.
  const master = new Map();
  let seq = 0;
  const strip = (doc) => {
    if (!doc) return undefined;
    const { _meta, _rev, ...rest } = doc;
    return JSON.stringify(rest);
  };

  async function device(name) {
    const db = await createRxDatabase({ name, storage: getRxStorageMemory() });
    await db.addCollections({
      discussions: {
        schema: {
          version: 0,
          primaryKey: "id",
          type: "object",
          properties: {
            id: { type: "string", maxLength: 40 },
            title: { type: "string" },
            updatedAt: { type: "number" },
          },
          required: ["id", "title", "updatedAt"],
        },
      },
      posts: { schema: postSchema(0) },
    });
    const sync = (collection) =>
      replicateRxCollection({
        collection,
        replicationIdentifier: `${name}-${collection.name}`,
        live: true,
        autoStart: false,
        push: {
          async handler(rows) {
            const conflicts = [];
            for (const { assumedMasterState, newDocumentState } of rows) {
              const key = `${collection.name}/${newDocumentState.id}`;
              const current = master.get(key)?.doc;
              if (current && strip(assumedMasterState) !== strip(current)) {
                conflicts.push(current);
              } else {
                seq += 1;
                master.set(key, { doc: newDocumentState, seq });
              }
            }
            return conflicts;
          },
        },
        pull: {
          async handler(checkpoint) {
            const after = checkpoint?.seq ?? 0;
            const entries = [...master.entries()]
              .filter(([k, e]) => k.startsWith(`${collection.name}/`) && e.seq > after)
              .map(([, e]) => e)
              .sort((x, y) => x.seq - y.seq);
            return {
              documents: entries.map((e) => e.doc),
              checkpoint: entries.length ? { seq: lastOfArray(entries).seq } : checkpoint,
            };
          },
        },
      });
    return { db, sync };
  }

  const a = await device("phone");
  const b = await device("desktop");

  // Shared starting point.
  await a.db.discussions.insert({ id: "d1", title: "CouchDB?", updatedAt: 1 });
  await a.db.posts.insert({ id: "root", discussionId: "d1", parentId: null, body: "root", createdAt: 1 });
  for (const device of [a, b]) {
    for (const c of [device.db.discussions, device.db.posts]) {
      const r = device.sync(c);
      await r.start();
      await r.awaitInSync();
      await r.cancel();
    }
  }

  // Both offline: each adds a reply and renames the discussion.
  await a.db.posts.insert({ id: "a1", discussionId: "d1", parentId: "root", body: "from phone", createdAt: 2 });
  await b.db.posts.insert({ id: "b1", discussionId: "d1", parentId: "root", body: "from desktop", createdAt: 3 });
  await (await a.db.discussions.findOne("d1").exec()).patch({ title: "Phone title", updatedAt: 2 });
  await (await b.db.discussions.findOne("d1").exec()).patch({ title: "Desktop title", updatedAt: 5 });

  // Phone reconnects first, then desktop.
  for (const device of [a, b, a]) {
    for (const c of [device.db.discussions, device.db.posts]) {
      const r = device.sync(c);
      await r.start();
      await r.awaitInSync();
      await r.cancel();
    }
  }

  const posts = (await b.db.posts.find().exec()).map((p) => p.id).sort();
  const titleA = (await a.db.discussions.findOne("d1").exec()).title;
  const titleB = (await b.db.discussions.findOne("d1").exec()).title;
  await a.db.close();
  await b.db.close();
  const masterTitle = JSON.parse(strip(master.get("discussions/d1").doc)).title;
  const newerWon = titleA === "Desktop title" && titleB === "Desktop title";
  report("Offline replication with the default conflict handler", [
    `posts after sync on desktop: ${posts.join(", ")}`,
    `title on phone: "${titleA}", on desktop: "${titleB}", on master: "${masterTitle}"`,
    "phone (older rename, updatedAt 2) reached the master first; desktop (newer, updatedAt 5) second",
    newerWon
      ? "the newer rename won"
      : "the newer rename was dropped: the default handler keeps the master state, and nothing tells the user",
  ]);
}

console.log(`# RxDB ${JSON.parse(await (await import("node:fs/promises")).readFile(new URL("./node_modules/rxdb/package.json", import.meta.url))).version} spike, Node ${process.version}`);
await trialLimits();
await migration();
await attachments();
await replicationConflicts();

// Live replication and the storage keep handles open; the experiments are done.
process.exit(0);
