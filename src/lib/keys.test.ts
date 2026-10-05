import { describe, expect, it } from "vitest";
import { adjacentPost, isTypingTarget } from "./keys";

describe("adjacentPost", () => {
  const ids = ["a", "b", "c"];

  it("moves forward and back, stopping at the ends", () => {
    expect(adjacentPost(ids, "a", 1)).toBe("b");
    expect(adjacentPost(ids, "b", -1)).toBe("a");
    expect(adjacentPost(ids, "c", 1)).toBe("c");
    expect(adjacentPost(ids, "a", -1)).toBe("a");
  });

  it("starts at the first or last post when none is focused", () => {
    expect(adjacentPost(ids, null, 1)).toBe("a");
    expect(adjacentPost(ids, null, -1)).toBe("c");
    expect(adjacentPost([], null, 1)).toBeUndefined();
  });
});

describe("isTypingTarget", () => {
  it("recognises text entry elements", () => {
    expect(isTypingTarget(document.createElement("textarea"))).toBe(true);
    expect(isTypingTarget(document.createElement("input"))).toBe(true);
    expect(isTypingTarget(document.createElement("article"))).toBe(false);
    expect(isTypingTarget(null)).toBe(false);
  });
});
