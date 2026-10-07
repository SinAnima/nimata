import { describe, expect, it } from "vitest";
import { fold, highlightParts, markedParts, unmarked } from "./search";

const marked = (parts: { text: string; match: boolean }[]) =>
  parts.map((p) => (p.match ? `[${p.text}]` : p.text)).join("");

describe("search highlighting", () => {
  it("folds case and accents like the index", () => {
    expect(fold("Νήματα ΆΈΉ λόγος Café")).toBe("νηματα αεη λογοσ cafe");
  });

  it("splits snippets marked by Rust", () => {
    const parts = markedParts("Could Datalog replace it?");
    expect(marked(parts)).toBe("Could [Datalog] replace [it]?");
    expect(unmarked("a b")).toBe("a b");
  });

  it("highlights word beginnings and phrases across accents", () => {
    expect(
      marked(
        highlightParts("Νήματα are threads; the Oracle rules.", [
          "νηματα",
          "the oracle",
        ]),
      ),
    ).toBe("[Νήματα] are threads; [the Oracle] rules.");
    expect(marked(highlightParts("stratification, strata", ["strat"]))).toBe(
      "[stratification], [strata]",
    );
    expect(marked(highlightParts("unchanged", []))).toBe("unchanged");
    expect(marked(highlightParts("the other", ["the oracle"]))).toBe(
      "the other",
    );
  });
});
