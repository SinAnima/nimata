import * as api from "../api";
import type {
  DiscussionFilter,
  DiscussionSummary,
  DiscussionView,
  ModelParticipant,
  Participant,
  Post,
  ProviderView,
  Uuid,
} from "../types";
import { Composers } from "./composer.svelte";
import { describeImport } from "../importReport";
import { askToConfirm } from "./confirmation.svelte";
import { nav } from "./nav.svelte";

/** Application data shown in the UI, loaded from and written through Rust. */
class Notebook {
  me: Participant | null = $state(null);
  filter: DiscussionFilter = $state("active");
  discussions: DiscussionSummary[] = $state([]);
  view: DiscussionView | null = $state(null);
  /** Set when the database cannot be used at all. */
  fatal: string | null = $state(null);
  /** The most recent failed action, shown until dismissed. */
  notice: string | null = $state(null);
  /** The outcome of the most recent successful file action, e.g. a backup. */
  status: string | null = $state(null);
  /** The post being edited in place, if any. */
  editingPostId: Uuid | null = $state(null);
  /** Providers with their key status and models, as shown in Settings. */
  providers: ProviderView[] = $state([]);
  /** The model that answers when neither a mention nor the thread decides. */
  defaultModelId: Uuid | null = $state(null);

  /** Every model participant, enabled or not. */
  get models(): ModelParticipant[] {
    return this.providers.flatMap((p) => p.models);
  }

  /** Models that can be asked to reply right now. */
  get askableModels(): ModelParticipant[] {
    // A provider that needs a key can answer only once it has one.
    const ready = new Set(
      this.providers
        .filter((p) => !p.capabilities.requiresKey || p.key.source !== null)
        .map((p) => p.provider.id),
    );
    return this.models.filter((m) => m.enabled && ready.has(m.providerId));
  }

  composers = new Composers(api.saveDraft, (e) => this.report(e));

  report(error: unknown): void {
    this.notice = api.errorMessage(error);
  }

  async init(wide: boolean): Promise<void> {
    try {
      this.me = await api.localUser();
      await this.refreshList();
    } catch (e) {
      this.fatal = api.errorMessage(e);
      return;
    }
    await this.loadProviders();
    const first = this.discussions[0];
    if (wide && first) await this.open(first.id);
  }

  async loadProviders(): Promise<void> {
    try {
      this.providers = await api.providers();
      this.defaultModelId = await api.defaultModel();
    } catch (e) {
      this.report(e);
    }
  }

  /**
   * Keeps the open discussion in step with replies being written. Returns a
   * function that stops listening.
   */
  async listenForReplies(): Promise<() => void> {
    const stopDeltas = await api.onPostDelta(({ postId, text }) => {
      const post = this.view?.posts.find((p) => p.id === postId);
      if (post) post.body += text;
    });
    const stopUpdates = await api.onPostUpdated((post) => {
      this.#showPost(post);
      if (post.status !== "streaming") void this.refreshList().catch(() => {});
    });
    return () => {
      stopDeltas();
      stopUpdates();
    };
  }

  /** Adds or replaces a post in the open discussion, if it belongs there. */
  #showPost(post: Post): void {
    const view = this.view;
    if (!view || view.discussion.id !== post.discussionId) return;
    const index = view.posts.findIndex((p) => p.id === post.id);
    if (index >= 0) view.posts[index] = post;
    else view.posts.push(post);
    if (!view.participants.some((p) => p.id === post.authorId)) {
      const model = this.models.find((m) => m.participant.id === post.authorId);
      if (model) view.participants.push(model.participant);
    }
  }

  async askModel(parentId: Uuid, participantId: Uuid): Promise<Post | null> {
    const view = this.view;
    if (!view) return null;
    try {
      const post = await api.askModel(
        view.discussion.id,
        parentId,
        participantId,
      );
      this.#showPost(post);
      return post;
    } catch (e) {
      this.report(e);
      return null;
    }
  }

  async cancelReply(postId: Uuid): Promise<void> {
    try {
      await api.cancelReply(postId);
    } catch (e) {
      this.report(e);
    }
  }

  async retryReply(postId: Uuid): Promise<Post | null> {
    try {
      const post = await api.retryReply(postId);
      this.#showPost(post);
      return post;
    } catch (e) {
      this.report(e);
      return null;
    }
  }

  async refreshList(): Promise<void> {
    this.discussions = await api.listDiscussions(this.filter);
  }

  async showFilter(filter: DiscussionFilter): Promise<void> {
    this.filter = filter;
    await this.refreshList().catch((e) => this.report(e));
  }

  async open(id: Uuid): Promise<void> {
    nav.open(id);
    try {
      const view = await api.getDiscussion(id);
      if (nav.selectedId !== id) return;
      this.composers.load(id, view.draft);
      this.view = view;
    } catch (e) {
      this.report(e);
    }
  }

  /** Starts a discussion, then asks `ask` to answer its first post. */
  async start(
    title: string,
    body: string,
    ask: ModelParticipant[] = [],
  ): Promise<boolean> {
    try {
      const view = await api.startDiscussion(title, body);
      this.view = view;
      nav.open(view.discussion.id);
      this.filter = "active";
      await this.refreshList();
      const first = view.posts[0];
      for (const model of first ? ask : []) {
        await this.askModel(first!.id, model.participant.id);
      }
      return true;
    } catch (e) {
      this.report(e);
      return false;
    }
  }

  /**
   * Posts the composer's draft as a reply to `parentId`, then asks each of
   * `ask` to answer it.
   */
  async post(
    parentId: Uuid,
    ask: ModelParticipant[] = [],
  ): Promise<Post | null> {
    const view = this.view;
    if (!view || !this.me) return null;
    const id = view.discussion.id;
    const { draft, context } = this.composers.get(id);
    try {
      const post = await api.addPost(id, parentId, draft, context);
      this.composers.clear(id);
      if (this.view?.discussion.id === id) {
        this.view.posts.push(post);
        if (!this.view.participants.some((p) => p.id === post.authorId)) {
          this.view.participants.push(this.me);
        }
      }
      await this.refreshList();
      for (const model of ask) {
        await this.askModel(post.id, model.participant.id);
      }
      return post;
    } catch (e) {
      this.report(e);
      return null;
    }
  }

  async rename(title: string): Promise<boolean> {
    if (!this.view) return false;
    try {
      this.view.discussion = await api.renameDiscussion(
        this.view.discussion.id,
        title,
      );
      await this.refreshList();
      return true;
    } catch (e) {
      this.report(e);
      return false;
    }
  }

  async setArchived(archived: boolean): Promise<void> {
    if (!this.view) return;
    try {
      this.view.discussion = await api.setArchived(
        this.view.discussion.id,
        archived,
      );
      await this.refreshList();
    } catch (e) {
      this.report(e);
    }
  }

  #replacePost(post: Post): void {
    const posts = this.view?.posts;
    const index = posts?.findIndex((p) => p.id === post.id) ?? -1;
    if (posts && index >= 0) posts[index] = post;
  }

  /** Replaces a post's text; the previous text is kept as a revision. */
  async editPost(postId: Uuid, body: string): Promise<boolean> {
    try {
      this.#replacePost(await api.editPost(postId, body));
      this.editingPostId = null;
      return true;
    } catch (e) {
      this.report(e);
      return false;
    }
  }

  async deletePost(postId: Uuid): Promise<void> {
    const confirmed = await askToConfirm(
      "Delete this post?",
      "Its text and any earlier versions are erased. Replies stay, shown as replying to a deleted post.",
      "Delete post",
    );
    if (!confirmed) return;
    try {
      this.#replacePost(await api.deletePost(postId));
      await this.refreshList();
    } catch (e) {
      this.report(e);
    }
  }

  async deleteDiscussion(): Promise<void> {
    const view = this.view;
    if (!view) return;
    const confirmed = await askToConfirm(
      `Delete “${view.discussion.title}”?`,
      "The discussion, all its posts, and its draft are erased from this device. Back up first if you might want it later.",
      "Delete discussion",
    );
    if (!confirmed) return;
    try {
      await api.deleteDiscussion(view.discussion.id);
      this.composers.clear(view.discussion.id);
      this.view = null;
      nav.selectedId = null;
      nav.back();
      await this.refreshList();
    } catch (e) {
      this.report(e);
    }
  }

  async exportDiscussion(): Promise<void> {
    if (!this.view) return;
    try {
      const path = await api.exportDiscussion(this.view.discussion.id);
      if (path) this.status = `Exported to ${path}`;
    } catch (e) {
      this.report(e);
    }
  }

  async exportMarkdown(): Promise<void> {
    if (!this.view) return;
    try {
      const path = await api.exportMarkdown(this.view.discussion.id);
      if (path) this.status = `Exported to ${path}`;
    } catch (e) {
      this.report(e);
    }
  }

  async exportArchive(): Promise<void> {
    try {
      const done = await api.exportArchive();
      if (done) {
        const n = done.discussions;
        this.status = `Exported ${n} ${n === 1 ? "discussion" : "discussions"} to ${done.path}`;
      }
    } catch (e) {
      this.report(e);
    }
  }

  /** Imports a file, then shows what changed. */
  async importFile(): Promise<void> {
    try {
      const report = await api.importFile();
      if (!report) return;
      this.status = describeImport(report);
      await this.refreshList();
      // A discussion that is open may have gained posts.
      const open = this.view?.discussion.id;
      if (open && report.outcomes.some((o) => o.discussionId === open)) {
        await this.open(open);
      }
    } catch (e) {
      this.report(e);
    }
  }

  async backup(): Promise<string | null> {
    try {
      const path = await api.backupDatabase();
      if (path) this.status = `Backed up to ${path}`;
      return path;
    } catch (e) {
      this.report(e);
      return null;
    }
  }

  /**
   * Replaces all discussions with a backup, then reloads everything. Works
   * even when the database could not be opened.
   */
  async restore(): Promise<boolean> {
    const confirmed = await askToConfirm(
      "Restore from a backup?",
      "Everything currently in Nimata is replaced by the backup. A safety copy of the current data is kept in the data folder first.",
      "Choose backup",
    );
    if (!confirmed) return false;
    try {
      const path = await api.restoreDatabase();
      if (!path) return false;
      this.fatal = null;
      this.view = null;
      nav.selectedId = null;
      nav.back();
      await this.init(matchMedia("(min-width: 48rem)").matches);
      if (!this.fatal) this.status = `Restored from ${path}`;
      return true;
    } catch (e) {
      if (this.fatal) this.fatal = api.errorMessage(e);
      else this.report(e);
      return false;
    }
  }

  async renameMe(name: string): Promise<boolean> {
    try {
      const me = await api.renameLocalUser(name);
      this.me = me;
      const author = this.view?.participants.find((p) => p.id === me.id);
      if (author) author.displayName = me.displayName;
      return true;
    } catch (e) {
      this.report(e);
      return false;
    }
  }
}

export const notebook = new Notebook();
