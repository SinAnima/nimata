import * as api from "../api";
import type {
  DiscussionFilter,
  DiscussionSummary,
  DiscussionView,
  Participant,
  Post,
  Uuid,
} from "../types";
import { Composers } from "./composer.svelte";
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
    const first = this.discussions[0];
    if (wide && first) await this.open(first.id);
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

  async start(title: string, body: string): Promise<boolean> {
    try {
      const view = await api.startDiscussion(title, body);
      this.view = view;
      nav.open(view.discussion.id);
      this.filter = "active";
      await this.refreshList();
      return true;
    } catch (e) {
      this.report(e);
      return false;
    }
  }

  /** Posts the composer's draft as the local user. */
  async post(): Promise<Post | null> {
    const view = this.view;
    if (!view || !this.me) return null;
    const id = view.discussion.id;
    const { replyTo, draft } = this.composers.get(id);
    try {
      const post = await api.addPost(id, replyTo, draft);
      this.composers.clear(id);
      if (this.view?.discussion.id === id) {
        this.view.posts.push(post);
        if (!this.view.participants.some((p) => p.id === post.authorId)) {
          this.view.participants.push(this.me);
        }
      }
      await this.refreshList();
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
