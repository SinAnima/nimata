import type { Uuid } from "../types";

/** Per-discussion composer state: the post being replied to and the draft. */
export interface ComposerState {
  replyTo: Uuid | null;
  draft: string;
}

const EMPTY: ComposerState = Object.freeze({ replyTo: null, draft: "" });

/** Kept per discussion so switching discussions does not lose a draft. */
export class Composers {
  #byDiscussion: Record<Uuid, ComposerState> = $state({});

  get(discussionId: Uuid): ComposerState {
    return this.#byDiscussion[discussionId] ?? EMPTY;
  }

  setReplyTo(discussionId: Uuid, postId: Uuid | null): void {
    this.#byDiscussion[discussionId] = {
      ...this.get(discussionId),
      replyTo: postId,
    };
  }

  setDraft(discussionId: Uuid, draft: string): void {
    this.#byDiscussion[discussionId] = { ...this.get(discussionId), draft };
  }
}

export const composers = new Composers();
