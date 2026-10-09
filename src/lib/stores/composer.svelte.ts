import type { Draft, StagedAttachment, Uuid } from "../types";

/** Per-discussion composer state: the post being replied to and the draft. */
export interface ComposerState {
  replyTo: Uuid | null;
  draft: string;
  /** Posts chosen as context, in the order chosen. */
  context: Uuid[];
  /** Files added, already stored, in the order added. */
  attachments: StagedAttachment[];
}

export type SaveDraft = (
  discussionId: Uuid,
  parentId: Uuid | null,
  body: string,
  contextIds: Uuid[],
  attachments: StagedAttachment[],
) => Promise<void>;

const EMPTY: ComposerState = Object.freeze({
  replyTo: null,
  draft: "",
  context: [],
  attachments: [],
});

export const SAVE_DELAY_MS = 400;

/**
 * Composer state per discussion. Changes are saved to the database shortly
 * after typing stops, so drafts survive navigation and restarts.
 */
export class Composers {
  #byDiscussion: Record<Uuid, ComposerState> = $state({});
  #pending = new Map<Uuid, ReturnType<typeof setTimeout>>();
  #save: SaveDraft;
  #onError: (error: unknown) => void;

  constructor(save: SaveDraft, onError: (error: unknown) => void) {
    this.#save = save;
    this.#onError = onError;
  }

  get(discussionId: Uuid): ComposerState {
    return this.#byDiscussion[discussionId] ?? EMPTY;
  }

  /** Adopts the stored draft unless this session already has newer state. */
  load(discussionId: Uuid, draft: Draft | null): void {
    if (this.#byDiscussion[discussionId] || !draft) return;
    this.#byDiscussion[discussionId] = {
      replyTo: draft.parentId,
      draft: draft.body,
      context: draft.contextIds ?? [],
      attachments: draft.attachments ?? [],
    };
  }

  setReplyTo(discussionId: Uuid, postId: Uuid | null): void {
    this.#update(discussionId, { replyTo: postId });
  }

  setDraft(discussionId: Uuid, draft: string): void {
    this.#update(discussionId, { draft });
  }

  /** Adds a post as context, or removes it if it is already there. */
  toggleContext(discussionId: Uuid, postId: Uuid): void {
    const context = this.get(discussionId).context;
    this.#update(discussionId, {
      context: context.includes(postId)
        ? context.filter((id) => id !== postId)
        : [...context, postId],
    });
  }

  addAttachment(discussionId: Uuid, file: StagedAttachment): void {
    this.#update(discussionId, {
      attachments: [...this.get(discussionId).attachments, file],
    });
  }

  /**
   * Takes out the file at `index`. The draft is saved at once, without it,
   * so its bytes can be deleted; returns the file.
   */
  async removeAttachment(
    discussionId: Uuid,
    index: number,
  ): Promise<StagedAttachment | undefined> {
    const files = this.get(discussionId).attachments;
    const removed = files[index];
    this.#update(discussionId, {
      attachments: files.filter((_, i) => i !== index),
    });
    await this.#saveNow(discussionId);
    return removed;
  }

  /** Forgets the composer after posting. The database clears its copy itself. */
  clear(discussionId: Uuid): void {
    clearTimeout(this.#pending.get(discussionId));
    this.#pending.delete(discussionId);
    delete this.#byDiscussion[discussionId];
  }

  /** Saves pending changes now, e.g. when the app is being hidden or closed. */
  flush(): void {
    for (const discussionId of [...this.#pending.keys()]) {
      void this.#saveNow(discussionId);
    }
  }

  #update(discussionId: Uuid, change: Partial<ComposerState>): void {
    this.#byDiscussion[discussionId] = { ...this.get(discussionId), ...change };
    clearTimeout(this.#pending.get(discussionId));
    this.#pending.set(
      discussionId,
      setTimeout(() => void this.#saveNow(discussionId), SAVE_DELAY_MS),
    );
  }

  async #saveNow(discussionId: Uuid): Promise<void> {
    clearTimeout(this.#pending.get(discussionId));
    this.#pending.delete(discussionId);
    const { replyTo, draft, context, attachments } = this.get(discussionId);
    await this.#save(discussionId, replyTo, draft, context, attachments).catch(
      this.#onError,
    );
  }
}
