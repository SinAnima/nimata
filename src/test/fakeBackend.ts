// An in-memory stand-in for the Rust commands, installed through Tauri's
// IPC mock so the real api.ts runs. It follows the rules of
// SqliteRepository (validation, ordering, draft handling, error messages);
// the Rust IPC tests check the real commands against the same shapes.

import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type {
  Discussion,
  DiscussionSummary,
  DiscussionView,
  Draft,
  Participant,
  Post,
} from "../lib/types";

type Args = Record<string, unknown>;

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
  calls: { cmd: string; args: Args }[] = [];
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
      status: "complete",
    };
    this.posts.push(post);
    return post;
  }

  view(id: string): DiscussionView {
    const posts = this.posts.filter((p) => p.discussionId === id);
    return {
      discussion: { ...this.#discussion(id) },
      posts,
      participants: posts.length > 0 ? [{ ...this.me }] : [],
      draft: this.drafts.get(id) ?? null,
    };
  }

  handle(cmd: string, args: Args): unknown {
    this.calls.push({ cmd, args });
    const failure = this.failures.get(cmd);
    if (failure !== undefined) {
      this.failures.delete(cmd);
      throw failure;
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
            const posts = this.posts.filter((p) => p.discussionId === d.id);
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
