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
