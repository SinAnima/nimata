// @mentions: asking models by short names. Rules match nimata-core's
// `automatic_alias` and alias validation.

import type { ModelParticipant } from "./types";

/** "GPT-6-sol" -> "gpt6sol": lowercase letters and digits of the name. */
export function automaticAlias(displayName: string): string {
  return Array.from(displayName.toLowerCase())
    .filter((c) => /[\p{L}\p{N}]/u.test(c))
    .join("");
}

/** Every alias a model answers to: its own aliases, then the automatic one. */
export function aliasesOf(model: ModelParticipant): string[] {
  const all = [...model.aliases, automaticAlias(model.participant.displayName)];
  return [...new Set(all.filter((a) => a !== ""))];
}

export interface Mention {
  alias: string;
  /** Where "@alias" starts and ends in the text. */
  start: number;
  end: number;
}

// "@" at the start or after a non-word character, so e-mail addresses are
// not mentions. A trailing "." "-" or "_" is punctuation, not part of it.
const MENTION = /(^|[^\p{L}\p{N}_@])@([\p{L}\p{N}][\p{L}\p{N}._-]*)/gu;

export function mentionsIn(text: string): Mention[] {
  const found: Mention[] = [];
  for (const match of text.matchAll(MENTION)) {
    const alias = match[2]!.replace(/[._-]+$/, "").toLowerCase();
    const start = match.index! + match[1]!.length;
    found.push({ alias, start, end: start + 1 + alias.length });
  }
  return found;
}

export type Resolution =
  | { kind: "model"; alias: string; model: ModelParticipant }
  | { kind: "ambiguous"; alias: string; candidates: ModelParticipant[] }
  | { kind: "unknown"; alias: string };

/**
 * Finds the model an alias names: an exact alias first, otherwise a prefix
 * that only one model's aliases start with ("@r" for "review").
 */
export function resolveAlias(
  alias: string,
  models: ModelParticipant[],
): Resolution {
  const name = alias.toLowerCase();
  const exact = models.filter((m) => aliasesOf(m).includes(name));
  if (exact.length === 1) return { kind: "model", alias, model: exact[0]! };
  if (exact.length > 1) return { kind: "ambiguous", alias, candidates: exact };
  const prefixed = models.filter((m) =>
    aliasesOf(m).some((a) => a.startsWith(name)),
  );
  if (prefixed.length === 1)
    return { kind: "model", alias, model: prefixed[0]! };
  if (prefixed.length > 1)
    return { kind: "ambiguous", alias, candidates: prefixed };
  return { kind: "unknown", alias };
}

export interface MentionResult {
  /** Models mentioned, each once, in the order first mentioned. */
  models: ModelParticipant[];
  /** Mentions that do not name exactly one model. */
  problems: Exclude<Resolution, { kind: "model" }>[];
}

export function resolveMentions(
  text: string,
  models: ModelParticipant[],
): MentionResult {
  const result: MentionResult = { models: [], problems: [] };
  for (const mention of mentionsIn(text)) {
    const resolution = resolveAlias(mention.alias, models);
    if (resolution.kind !== "model") result.problems.push(resolution);
    else if (!result.models.includes(resolution.model))
      result.models.push(resolution.model);
  }
  return result;
}

/** The partial "@ali" ending at `cursor`, if the cursor is inside one. */
export function mentionAt(text: string, cursor: number): Mention | undefined {
  return mentionsIn(text.slice(0, cursor)).find((m) => m.end === cursor);
}

/** Explains a mention problem in a sentence. */
export function describeProblem(
  problem: MentionResult["problems"][number],
): string {
  if (problem.kind === "unknown")
    return `@${problem.alias} does not name a model. Add it as an alias in Settings, Models.`;
  const names = problem.candidates.map((m) => m.participant.displayName);
  return `@${problem.alias} could mean ${names.join(" or ")}. Type more of the name.`;
}
