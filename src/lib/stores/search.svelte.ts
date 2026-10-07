import * as api from "../api";
import type { DiscussionHit, PostHit, SearchResults, Uuid } from "../types";
import { notebook } from "./notebook.svelte";

/** Waits this long after typing stops before searching. */
const TYPING_PAUSE_MS = 150;

/** Search across all discussions, shown in place of the discussion list. */
class Search {
  query = $state("");
  results: SearchResults | null = $state(null);
  recent: string[] = $state([]);
  /** Words to highlight, and the discussion they apply to. */
  highlight: string[] = $state([]);
  highlightIn: Uuid | null = $state(null);
  /** A post to scroll to once its discussion is shown. */
  target: Uuid | null = $state(null);

  #timer: ReturnType<typeof setTimeout> | undefined;
  /** Ignores results that arrive after a newer search started. */
  #latest = 0;

  get active(): boolean {
    return this.query.trim() !== "";
  }

  type(query: string): void {
    this.query = query;
    clearTimeout(this.#timer);
    if (!this.active) {
      this.results = null;
      return;
    }
    this.#timer = setTimeout(() => void this.run(), TYPING_PAUSE_MS);
  }

  async run(): Promise<void> {
    clearTimeout(this.#timer);
    const query = this.query;
    const id = ++this.#latest;
    if (!this.active) return;
    try {
      const results = await api.search(query);
      if (id === this.#latest) this.results = results;
    } catch (e) {
      notebook.report(e);
    }
  }

  async loadRecent(): Promise<void> {
    try {
      this.recent = await api.recentSearches();
    } catch (e) {
      notebook.report(e);
    }
  }

  async #remember(): Promise<void> {
    try {
      this.recent = await api.recordSearch(this.query);
    } catch (e) {
      notebook.report(e);
    }
  }

  async openPost(hit: PostHit): Promise<void> {
    this.highlight = this.results?.highlight ?? [];
    this.highlightIn = hit.discussionId;
    this.target = hit.postId;
    void this.#remember();
    await notebook.open(hit.discussionId);
  }

  async openDiscussion(hit: DiscussionHit): Promise<void> {
    this.highlight = this.results?.highlight ?? [];
    this.highlightIn = hit.discussionId;
    this.target = null;
    void this.#remember();
    await notebook.open(hit.discussionId);
  }

  async useRecent(query: string): Promise<void> {
    this.query = query;
    await this.run();
  }

  async clearRecent(): Promise<void> {
    try {
      await api.clearRecentSearches();
      this.recent = [];
    } catch (e) {
      notebook.report(e);
    }
  }

  /** Leaves search: the list returns and highlights go away. */
  clear(): void {
    clearTimeout(this.#timer);
    this.#latest++;
    this.query = "";
    this.results = null;
    this.highlight = [];
    this.highlightIn = null;
    this.target = null;
  }
}

export const search = new Search();
