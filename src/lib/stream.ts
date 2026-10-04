import type { Participant, Post, Uuid } from "./types";

/** Posts in the order they were written. Ties keep their input order. */
export function chronological(posts: readonly Post[]): Post[] {
  return [...posts].sort((a, b) => a.createdAt - b.createdAt);
}

export function indexById<T extends { id: Uuid }>(
  items: readonly T[],
): Map<Uuid, T> {
  return new Map(items.map((item) => [item.id, item]));
}

/**
 * Whether the reply reference should quote its parent. Quoting is redundant
 * when the parent is the post directly above.
 */
export function shouldQuoteParent(
  post: Post,
  previous: Post | undefined,
): boolean {
  return post.parentId !== null && post.parentId !== previous?.id;
}

export function authorName(
  participants: Map<Uuid, Participant>,
  id: Uuid,
): string {
  return participants.get(id)?.displayName ?? "Unknown participant";
}

/** First non-empty line of a body, cut to `max` characters. */
export function excerpt(body: string, max = 90): string {
  const line =
    body
      .split("\n")
      .find((l) => l.trim() !== "")
      ?.trim() ?? "";
  const chars = Array.from(line);
  return chars.length <= max
    ? line
    : `${chars
        .slice(0, max - 1)
        .join("")
        .trimEnd()}…`;
}

export function paragraphs(body: string): string[] {
  return body
    .split(/\n\s*\n/)
    .map((p) => p.trim())
    .filter((p) => p !== "");
}
