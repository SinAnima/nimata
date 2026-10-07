// Showing search matches: the marks Rust puts in snippets, and highlighting
// the same words in posts, folded the way the search index folds them.

/** Marks around matches in snippets and titles from Rust. */
export const MATCH_START = "";
export const MATCH_END = "";

export interface Part {
  text: string;
  match: boolean;
}

/** Lowercase without accents in any script, as in nimata_core::search. */
export function fold(text: string): string {
  return text
    .toLowerCase()
    .normalize("NFD")
    .replace(/\p{M}/gu, "")
    .replace(/ς/g, "σ");
}

/** Splits text marked by Rust into plain and matching parts. */
export function markedParts(text: string): Part[] {
  const parts: Part[] = [];
  const re = new RegExp(`${MATCH_START}([^${MATCH_END}]*)${MATCH_END}`, "g");
  let at = 0;
  for (const m of text.matchAll(re)) {
    if (m.index > at)
      parts.push({ text: text.slice(at, m.index), match: false });
    parts.push({ text: m[1] ?? "", match: true });
    at = m.index + m[0].length;
  }
  if (at < text.length) parts.push({ text: text.slice(at), match: false });
  return parts;
}

/** Text without match marks, e.g. for accessible names. */
export function unmarked(text: string): string {
  return text.replaceAll(MATCH_START, "").replaceAll(MATCH_END, "");
}

const WORD = /[\p{L}\p{N}]+/gu;

function wordsOf(text: string): string[] {
  return [...fold(text).matchAll(WORD)].map((m) => m[0]);
}

/**
 * Splits `text` into parts, marking words that begin with one of `terms`
 * (single letters only as whole words)
 * and runs of words that equal a multi-word term (a phrase).
 */
export function highlightParts(text: string, terms: string[]): Part[] {
  if (terms.length === 0) return [{ text, match: false }];
  const spans = [...text.matchAll(WORD)].map((m) => ({
    start: m.index,
    end: m.index + m[0].length,
    word: fold(m[0]),
  }));
  const hit = spans.map(() => false);
  for (const term of terms) {
    const words = wordsOf(term);
    if (words.length === 1) {
      // Single letters match only themselves, as in search.
      const [word] = words as [string];
      const exact = [...word].length === 1;
      spans.forEach((s, i) => {
        if (s.word === word || (!exact && s.word.startsWith(word))) {
          hit[i] = true;
        }
      });
    } else if (words.length > 1) {
      for (let i = 0; i + words.length <= spans.length; i++) {
        if (words.every((w, j) => spans[i + j]!.word === w)) {
          hit.fill(true, i, i + words.length);
        }
      }
    }
  }
  const parts: Part[] = [];
  let at = 0;
  spans.forEach((s, i) => {
    if (!hit[i]) return;
    // Join with the previous match when only spaces separate them.
    const last = parts.at(-1);
    const gap = text.slice(at, s.start);
    if (last?.match && /^\s*$/.test(gap) && at > 0) {
      last.text += gap + text.slice(s.start, s.end);
    } else {
      if (s.start > at) parts.push({ text: gap, match: false });
      parts.push({ text: text.slice(s.start, s.end), match: true });
    }
    at = s.end;
  });
  if (at < text.length) parts.push({ text: text.slice(at), match: false });
  return parts;
}
