// An in-memory stand-in for the Rust commands, installed through Tauri's
// IPC mock so the real api.ts runs. It follows the rules of
// SqliteRepository (validation, ordering, draft handling, error messages);
// the Rust IPC tests check the real commands against the same shapes.

import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import type {
  Discussion,
  DiscussionSummary,
  DiscussionView,
  Draft,
  Participant,
  Capabilities,
  Generation,
  KeyStatus,
  ModelParticipant,
  Post,
  ProviderConfig,
  Revision,
} from "../lib/types";

type Args = Record<string, unknown>;

/** Everything a backup contains. */
interface Snapshot {
  me: Participant;
  discussions: Discussion[];
  posts: Post[];
  drafts: [string, Draft][];
  revisions: Revision[];
}

export class FakeBackend {
  me: Participant = {
    id: "me",
    kind: "human",
    displayName: "Me",
    provider: null,
    model: null,
  };
  discussions: Discussion[] = [];
  posts: Post[] = [];
  drafts = new Map<string, Draft>();
  revisions: Revision[] = [];
  calls: { cmd: string; args: Args }[] = [];
  /** Backups written by backup_database, by path. */
  backups = new Map<string, Snapshot>();
  /** Files written by export_discussion, by path. */
  exports = new Map<string, string>();
  /** What the next file dialog returns: a path, or null for Cancel. */
  dialogPath: string | null = "/backups/nimata-backup.sqlite3";
  /** Set to make get/list fail as if the database could not be opened. */
  openError: string | null = null;

  openai: ProviderConfig = {
    id: "provider-openai",
    kind: "openai",
    displayName: "OpenAI",
    baseUrl: null,
  };
  anthropic: ProviderConfig = {
    id: "provider-anthropic",
    kind: "anthropic",
    displayName: "Anthropic",
    baseUrl: null,
  };
  /** Every connection: the standard two, then added ones. */
  connections: ProviderConfig[] = [this.openai, this.anthropic];
  /** Saved keys by provider ID, kept here only to check they never reach the UI. */
  keys = new Map<string, string>();

  /** The OpenAI key, which most tests use. */
  get savedKey(): string | null {
    return this.keys.get(this.openai.id) ?? null;
  }
  set savedKey(key: string | null) {
    if (key === null) this.keys.delete(this.openai.id);
    else this.keys.set(this.openai.id, key);
  }
  /** Shown in development builds. */
  developmentBuild = true;
  /** What "Test connection" finds, or an error to report. */
  availableModels: string[] | string = ["gpt-5.6", "gpt-5.6-mini"];
  models: ModelParticipant[] = [];
  generations: Generation[] = [];
  defaultModelId: string | null = null;
  /** Commands that should fail on their next call, with the error to return. */
  failures = new Map<string, string>();
  now = Date.UTC(2026, 9, 4, 16, 0);
  #ids = 0;

  install(): this {
    clearMocks();
    mockIPC((cmd, args) => this.handle(cmd, (args ?? {}) as Args), {
      shouldMockEvents: true,
    });
    return this;
  }

  failNext(cmd: string, message: string): void {
    this.failures.set(cmd, message);
  }

  #id(): string {
    this.#ids += 1;
    return `00000000-0000-7000-8000-${String(this.#ids).padStart(12, "0")}`;
  }

  #tick(): number {
    this.now += 60_000;
    return this.now;
  }

  #discussion(id: unknown): Discussion {
    const d = this.discussions.find((d) => d.id === id);
    if (!d) throw "discussion not found";
    return d;
  }

  #cleanBody(body: unknown): string {
    const text = String(body)
      .replace(/^[\r\n]+|[\r\n]+$/g, "")
      .trimEnd();
    if (text.trim() === "") throw "a post cannot be empty";
    return text;
  }

  #cleanTitle(title: unknown): string {
    const text = String(title).trim();
    if (text === "") throw "a discussion needs a title";
    return text;
  }

  #checkParent(discussionId: unknown, parentId: unknown): void {
    if (parentId === null || parentId === undefined) return;
    const parent = this.posts.find((p) => p.id === parentId);
    if (!parent) throw "post being replied to not found";
    if (parent.discussionId !== discussionId)
      throw "a post can only reply to a post in the same discussion";
    if (parent.deletedAt !== null) throw "a deleted post cannot be replied to";
  }

  #post(id: unknown): Post {
    const post = this.posts.find((p) => p.id === id);
    if (!post) throw "post not found";
    return post;
  }

  #snapshot(): Snapshot {
    return structuredClone({
      me: this.me,
      discussions: this.discussions,
      posts: this.posts,
      drafts: [...this.drafts],
      revisions: this.revisions,
    });
  }

  #load(snapshot: Snapshot): void {
    const copy = structuredClone(snapshot);
    this.me = copy.me;
    this.discussions = copy.discussions;
    this.posts = copy.posts;
    this.drafts = new Map(copy.drafts);
    this.revisions = copy.revisions;
  }

  #insertPost(
    discussionId: string,
    parentId: string | null,
    body: string,
  ): Post {
    const post: Post = {
      id: this.#id(),
      discussionId,
      parentId,
      authorId: this.me.id,
      body,
      createdAt: this.#tick(),
      tzOffsetMinutes: -240,
      editedAt: null,
      deletedAt: null,
      status: "complete",
      providerMetadata: null,
    };
    this.posts.push(post);
    return post;
  }

  view(id: string): DiscussionView {
    const posts = this.posts.filter((p) => p.discussionId === id);
    const authors = new Set(posts.map((p) => p.authorId));
    return {
      discussion: { ...this.#discussion(id) },
      posts,
      participants: [
        { ...this.me },
        ...this.models.map((m) => ({ ...m.participant })),
      ].filter((p) => authors.has(p.id)),
      draft: this.drafts.get(id) ?? null,
    };
  }

  #connection(id: unknown): ProviderConfig {
    const provider = this.connections.find((p) => p.id === id);
    if (!provider) throw "provider not found";
    return provider;
  }

  #capabilities(provider: ProviderConfig): Capabilities {
    const custom = provider.kind === "openai_compatible";
    return {
      requiresKey: !custom,
      customEndpoint: custom,
      multiple: custom,
      modelDiscovery: true,
      streaming: true,
      usage: !custom,
    };
  }

  #keyStatus(provider: ProviderConfig): KeyStatus {
    const key = this.keys.get(provider.id) ?? null;
    const variable = {
      openai: "OPENAI_API_KEY",
      anthropic: "ANTHROPIC_API_KEY",
    }[provider.kind as "openai" | "anthropic"];
    return {
      source: key ? "saved" : null,
      hint: key && this.developmentBuild ? key.slice(-4) : null,
      store: "the Keychain",
      environmentVariable: this.developmentBuild ? (variable ?? null) : null,
    };
  }

  #generation(postId: string): Generation {
    const generation = this.generations.find((g) => g.postId === postId);
    if (!generation) throw "reply not found";
    return generation;
  }

  #startReply(parentId: string, participantId: string): Post {
    const parent = this.#post(parentId);
    if (parent.status !== "complete")
      throw "a model can only reply to a finished post";
    const model = this.models.find((m) => m.participant.id === participantId);
    if (!model) throw "this model is no longer set up in Settings";
    const provider = this.#connection(model.providerId);
    if (this.#capabilities(provider).requiresKey && !this.keys.has(provider.id))
      throw `Add an ${provider.displayName} API key in Settings, Models.`;
    const post: Post = {
      ...this.#insertPost(parent.discussionId, parent.id, ""),
      authorId: model.participant.id,
      status: "streaming",
    };
    this.posts[this.posts.length - 1] = post;
    this.generations.push({
      id: `generation-${post.id}`,
      postId: post.id,
      participantId,
      status: "sending",
      error: null,
      contextPostIds: [parent.id],
      startedAt: post.createdAt,
      finishedAt: null,
    });
    return { ...post };
  }

  /** Streams text into a reply the way the Rust runner does. */
  async streamText(postId: string, ...pieces: string[]): Promise<void> {
    const post = this.#post(postId);
    for (const text of pieces) {
      post.body += text;
      await emit("nimata://post-delta", { postId, text });
    }
  }

  /** Ends a reply as complete, failed, or cancelled, and announces it. */
  async endReply(
    postId: string,
    status: "complete" | "failed" | "cancelled",
    error: string | null = null,
  ): Promise<void> {
    const post = this.#post(postId);
    const generation = this.#generation(postId);
    post.status = status;
    generation.status = status;
    generation.error = error;
    generation.finishedAt = this.#tick();
    if (status === "complete") {
      post.providerMetadata = {
        provider: "openai",
        model: "gpt-5.6-2026-08-01",
        responseId: "resp_1",
        requestId: "req_1",
        inputTokens: 41,
        outputTokens: 12,
        incompleteReason: null,
      };
    }
    await emit("nimata://post-updated", { ...post });
  }

  handle(cmd: string, args: Args): unknown {
    this.calls.push({ cmd, args });
    const failure = this.failures.get(cmd);
    if (failure !== undefined) {
      this.failures.delete(cmd);
      throw failure;
    }
    const databaseCommands = cmd !== "app_info" && !cmd.startsWith("plugin:");
    if (this.openError && databaseCommands && cmd !== "database_status") {
      if (cmd !== "restore_database") throw this.openError;
    }
    switch (cmd) {
      case "local_user":
        return { ...this.me };
      case "rename_local_user": {
        const name = String(args.name).trim();
        if (name === "") throw "names must be 1 to 80 characters";
        this.me.displayName = name;
        return { ...this.me };
      }
      case "list_discussions":
        return this.discussions
          .filter(
            (d) => (args.filter === "archived") === (d.archivedAt !== null),
          )
          .sort((a, b) => b.updatedAt - a.updatedAt)
          .map((d): DiscussionSummary => {
            const posts = this.posts.filter(
              (p) => p.discussionId === d.id && p.deletedAt === null,
            );
            return {
              id: d.id,
              title: d.title,
              lastActivityAt: d.updatedAt,
              postCount: posts.length,
              excerpt: posts.at(-1)?.body.split("\n")[0] ?? "",
            };
          });
      case "get_discussion":
        return this.view(String(args.id));
      case "start_discussion": {
        const body = this.#cleanBody(args.body);
        const title =
          String(args.title).trim() === ""
            ? this.#cleanTitle(body.split("\n").find((l) => l.trim()) ?? "")
            : this.#cleanTitle(args.title);
        const at = this.now + 60_000;
        const d: Discussion = {
          id: this.#id(),
          title,
          createdAt: at,
          updatedAt: at,
          archivedAt: null,
          pinnedAt: null,
        };
        this.discussions.push(d);
        this.#insertPost(d.id, null, body);
        return this.view(d.id);
      }
      case "add_post": {
        const d = this.#discussion(args.discussionId);
        const body = this.#cleanBody(args.body);
        if (args.parentId === null || args.parentId === undefined)
          throw "a post replies to an earlier post; start a new discussion for a new topic";
        this.#checkParent(d.id, args.parentId);
        const post = this.#insertPost(
          d.id,
          (args.parentId as string) ?? null,
          body,
        );
        d.updatedAt = post.createdAt;
        this.drafts.delete(d.id);
        return post;
      }
      case "rename_discussion": {
        const d = this.#discussion(args.id);
        d.title = this.#cleanTitle(args.title);
        return { ...d };
      }
      case "set_archived": {
        const d = this.#discussion(args.id);
        d.archivedAt = args.archived ? this.now : null;
        return { ...d };
      }
      case "save_draft": {
        const id = String(args.discussionId);
        const parentId = (args.parentId as string | null) ?? null;
        const body = String(args.body);
        if (body.trim() === "" && parentId === null) {
          this.drafts.delete(id);
          return null;
        }
        this.#discussion(id);
        this.#checkParent(id, parentId);
        this.drafts.set(id, {
          discussionId: id,
          parentId,
          body,
          updatedAt: this.now,
        });
        return null;
      }
      case "edit_post": {
        const post = this.#post(args.postId);
        if (post.deletedAt !== null) throw "a deleted post cannot be edited";
        const body = this.#cleanBody(args.body);
        if (body === post.body) return { ...post };
        const at = this.#tick();
        this.revisions.push({
          postId: post.id,
          body: post.body,
          writtenAt: post.editedAt ?? post.createdAt,
          replacedAt: at,
        });
        post.body = body;
        post.editedAt = at;
        return { ...post };
      }
      case "post_revisions":
        this.#post(args.postId);
        return this.revisions.filter((r) => r.postId === args.postId);
      case "delete_post": {
        const post = this.#post(args.postId);
        if (post.deletedAt === null) {
          post.body = "";
          post.deletedAt = this.#tick();
          this.revisions = this.revisions.filter((r) => r.postId !== post.id);
          for (const draft of this.drafts.values()) {
            if (draft.parentId === post.id) draft.parentId = null;
          }
        }
        return { ...post };
      }
      case "delete_discussion": {
        const d = this.#discussion(args.id);
        this.discussions = this.discussions.filter((x) => x.id !== d.id);
        this.posts = this.posts.filter((p) => p.discussionId !== d.id);
        this.drafts.delete(d.id);
        return null;
      }
      case "database_status":
        return { path: "/data/nimata.sqlite3", error: this.openError };
      case "export_discussion": {
        const view = this.view(String(args.id));
        if (this.dialogPath === null) return null;
        this.exports.set(
          this.dialogPath,
          JSON.stringify({ format: "nimata/1", ...view }),
        );
        return this.dialogPath;
      }
      case "backup_database":
        if (this.dialogPath === null) return null;
        this.backups.set(this.dialogPath, this.#snapshot());
        return this.dialogPath;
      case "restore_database": {
        if (this.dialogPath === null) return null;
        const backup = this.backups.get(this.dialogPath);
        if (!backup) throw `${this.dialogPath} is not a Nimata database`;
        this.#load(backup);
        this.openError = null;
        return this.dialogPath;
      }
      case "providers":
        return this.connections.map((provider) => ({
          provider: { ...provider },
          capabilities: this.#capabilities(provider),
          key: this.#keyStatus(provider),
          models: structuredClone(
            this.models.filter((m) => m.providerId === provider.id),
          ),
        }));
      case "save_api_key": {
        const provider = this.#connection(args.providerId);
        const key = String(args.key).trim();
        if (key === "") throw "paste the API key first";
        this.keys.set(provider.id, key);
        return this.#keyStatus(provider);
      }
      case "remove_api_key": {
        const provider = this.#connection(args.providerId);
        this.keys.delete(provider.id);
        return this.#keyStatus(provider);
      }
      case "provider_models": {
        const provider = this.#connection(args.providerId);
        if (
          this.#capabilities(provider).requiresKey &&
          !this.keys.has(provider.id)
        )
          throw `Add an ${provider.displayName} API key first.`;
        if (typeof this.availableModels === "string")
          throw this.availableModels;
        return this.availableModels.map((id) => ({ id }));
      }
      case "add_endpoint": {
        const name = String(args.displayName).trim();
        const url = String(args.baseUrl).trim();
        if (name === "") throw "names must be 1 to 80 characters";
        if (!/^https?:\/\//.test(url))
          throw "an endpoint must start with https:// or http://";
        const provider: ProviderConfig = {
          id: `provider-${this.#id()}`,
          kind: "openai_compatible",
          displayName: name,
          baseUrl: url,
        };
        this.connections.push(provider);
        if (args.key) this.keys.set(provider.id, String(args.key));
        return { ...provider };
      }
      case "update_endpoint": {
        const provider = this.#connection(args.providerId);
        provider.displayName = String(args.displayName).trim();
        provider.baseUrl = String(args.baseUrl).trim();
        return { ...provider };
      }
      case "remove_endpoint": {
        const provider = this.#connection(args.providerId);
        const ids = this.models
          .filter((m) => m.providerId === provider.id)
          .map((m) => m.participant.id);
        if (this.posts.some((p) => ids.includes(p.authorId)))
          throw `models from ${provider.displayName} have written posts, so it stays; turn its models off instead`;
        this.models = this.models.filter((m) => m.providerId !== provider.id);
        this.connections = this.connections.filter((p) => p !== provider);
        this.keys.delete(provider.id);
        return null;
      }
      case "set_model": {
        const provider = this.#connection(args.providerId);
        const model = String(args.model).trim();
        const name = String(args.displayName).trim();
        if (name === "") throw "names must be 1 to 80 characters";
        let entry = this.models.find(
          (m) => m.providerId === provider.id && m.participant.model === model,
        );
        if (entry) {
          entry.participant.displayName = name;
          entry.enabled = Boolean(args.enabled);
        } else {
          entry = {
            participant: {
              id: `model-${model}`,
              kind: "model",
              displayName: name,
              provider: provider.kind,
              model,
            },
            providerId: provider.id,
            enabled: Boolean(args.enabled),
            aliases: [],
          };
          this.models.push(entry);
        }
        return structuredClone(entry);
      }
      case "ask_model":
        return this.#startReply(
          String(args.parentId),
          String(args.participantId),
        );
      case "cancel_reply": {
        const post = this.#post(args.postId);
        if (post.status !== "streaming") return false;
        void this.endReply(post.id, "cancelled");
        return true;
      }
      case "retry_reply": {
        const generation = this.#generation(String(args.postId));
        const post = this.#post(args.postId);
        return this.#startReply(post.parentId!, generation.participantId);
      }
      case "set_model_aliases": {
        const model = this.models.find(
          (m) => m.participant.id === args.participantId,
        );
        if (!model) throw "model participant not found";
        const cleaned = (args.aliases as string[])
          .map((a) => a.trim().replace(/^@/, "").toLowerCase())
          .filter((a) => a !== "");
        for (const alias of cleaned) {
          if (/\s/.test(alias)) throw `"${alias}" cannot be an alias`;
          const other = this.models.find(
            (m) => m !== model && m.aliases.includes(alias),
          );
          if (other)
            throw `@${alias} already names ${other.participant.displayName}`;
        }
        model.aliases = [...new Set(cleaned)];
        return structuredClone(model);
      }
      case "default_model":
        return this.defaultModelId;
      case "set_default_model":
        this.defaultModelId = (args.participantId as string | null) ?? null;
        return null;
      case "reply_details":
        return this.generations.find((g) => g.postId === args.postId) ?? null;
      case "app_info":
        return {
          version: "0.1.0",
          tauriVersion: "2.0.0",
          databasePath: "/data/nimata.sqlite3",
          platform: "macos",
        };
    }
    if (cmd.startsWith("plugin:event|")) return 1;
    throw `unknown command ${cmd}`;
  }
}
