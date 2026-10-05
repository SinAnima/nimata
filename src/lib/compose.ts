// Who a new post replies to, and which models are asked to answer it.

import type { ModelParticipant, Post, Uuid } from "./types";
import { resolveMentions, type MentionResult } from "./mentions";

/** The post a new post replies to unless another is chosen: the latest one
 * that is not deleted and did not fail. */
export function defaultTarget(posts: readonly Post[]): Post | undefined {
  return [...posts]
    .filter(
      (p) =>
        p.deletedAt === null &&
        (p.status === "complete" || p.status === "streaming"),
    )
    .sort((a, b) => a.createdAt - b.createdAt)
    .at(-1);
}

/** "none" asks no one; a participant ID asks that model; null decides
 * automatically. */
export type AskChoice = "none" | Uuid | null;

export type AnswerReason =
  "mentioned" | "chosen" | "replying" | "last in thread" | "default" | "nobody";

export interface Answerers {
  models: ModelParticipant[];
  reason: AnswerReason;
  mentions: MentionResult;
}

/**
 * Decides who answers a new post, in order: models @mentioned in the text;
 * an explicit choice; the model being replied to; the model that last
 * replied in the discussion; the default model.
 */
export function whoAnswers(input: {
  text: string;
  choice: AskChoice;
  target: Post | undefined;
  posts: readonly Post[];
  askable: ModelParticipant[];
  defaultModelId: Uuid | null;
}): Answerers {
  const { text, choice, target, posts, askable, defaultModelId } = input;
  const mentions = resolveMentions(text, askable);
  const result = (models: ModelParticipant[], reason: AnswerReason) => ({
    models,
    reason,
    mentions,
  });
  const byId = (id: Uuid | null | undefined) =>
    askable.find((m) => m.participant.id === id);

  if (mentions.models.length > 0 || mentions.problems.length > 0)
    return result(mentions.models, "mentioned");
  if (choice === "none") return result([], "nobody");
  const chosen = byId(choice);
  if (chosen) return result([chosen], "chosen");
  const replying = byId(target?.authorId);
  if (replying) return result([replying], "replying");
  const last = [...posts]
    .sort((a, b) => b.createdAt - a.createdAt)
    .map((p) => byId(p.authorId))
    .find((m) => m !== undefined);
  if (last) return result([last], "last in thread");
  const fallback = byId(defaultModelId);
  if (fallback) return result([fallback], "default");
  return result([], "nobody");
}
