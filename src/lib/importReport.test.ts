import { describe, expect, it } from "vitest";
import { describeImport } from "./importReport";
import type { ImportOutcome } from "./types";

const outcome = (
  result: ImportOutcome["result"],
  postsAdded: number,
): ImportOutcome => ({
  discussionId: crypto.randomUUID(),
  title: "t",
  result,
  postsAdded,
  postsPresent: 0,
});

describe("describing an import", () => {
  it("counts new discussions, new posts, and what was already here", () => {
    expect(
      describeImport({
        source: "chatGpt",
        outcomes: [
          outcome("added", 12),
          outcome("added", 3),
          outcome("updated", 1),
          outcome("unchanged", 0),
          outcome("skippedDeleted", 0),
        ],
        failures: [],
      }),
    ).toBe(
      "Imported 2 new discussions (15 posts) and 1 new post in 1 discussion you already had from the ChatGPT export. 1 discussion was already here. 1 discussion you deleted was left out.",
    );
  });

  it("says when nothing was new, and why something failed", () => {
    expect(
      describeImport({
        source: "nimataArchive",
        outcomes: [outcome("unchanged", 0), outcome("unchanged", 0)],
        failures: ["Forged: post x already belongs to another discussion"],
      }),
    ).toBe(
      "Nothing new in the Nimata archive. 2 discussions were already here. 1 discussion could not be imported: Forged: post x already belongs to another discussion",
    );
  });
});
