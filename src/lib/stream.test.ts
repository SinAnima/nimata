import { describe, expect, it } from "vitest";
import {
  chronological,
  excerpt,
  paragraphs,
  shouldQuoteParent,
} from "./stream";
import type { Post } from "./types";

function post(
  id: string,
  createdAt: number,
  parentId: string | null = null,
): Post {
  return {
    id,
    discussionId: "d",
    parentId,
    authorId: "a",
    body: id,
    createdAt,
    tzOffsetMinutes: 0,
    editedAt: null,
    status: "complete",
  };
}

describe("chronological", () => {
  it("orders by creation time, not by reply tree", () => {
    // root -> a -> c, root -> b. Tree order would be root, a, c, b.
    const root = post("root", 1);
    const a = post("a", 2, "root");
    const b = post("b", 3, "root");
    const c = post("c", 4, "a");
    expect(chronological([c, root, b, a]).map((p) => p.id)).toEqual([
      "root",
      "a",
      "b",
      "c",
    ]);
  });
});

describe("shouldQuoteParent", () => {
  it("does not quote a parent that is directly above", () => {
    expect(shouldQuoteParent(post("b", 2, "a"), post("a", 1))).toBe(false);
  });

  it("quotes a parent that is further up the stream", () => {
    expect(shouldQuoteParent(post("c", 3, "a"), post("b", 2, "a"))).toBe(true);
  });

  it("never quotes for a root post", () => {
    expect(shouldQuoteParent(post("r", 1), undefined)).toBe(false);
  });
});

describe("text helpers", () => {
  it("excerpts the first non-empty line on character boundaries", () => {
    expect(excerpt("\n\nνήματα threads\nmore", 4)).toBe("νήμ…");
  });

  it("splits paragraphs on blank lines", () => {
    expect(paragraphs("one\nstill one\n\n  two  \n\n\n")).toEqual([
      "one\nstill one",
      "two",
    ]);
  });
});
