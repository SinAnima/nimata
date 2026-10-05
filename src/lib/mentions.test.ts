import { describe, expect, it } from "vitest";
import {
  automaticAlias,
  mentionAt,
  mentionsIn,
  resolveAlias,
  resolveMentions,
} from "./mentions";
import type { ModelParticipant } from "./types";

function model(name: string, aliases: string[] = []): ModelParticipant {
  return {
    participant: {
      id: `id-${name}`,
      kind: "model",
      displayName: name,
      provider: "openai",
      model: name.toLowerCase(),
    },
    providerId: "openai",
    enabled: true,
    aliases,
  };
}

const sol = model("GPT-6-sol", ["review", "r"]);
const research = model("Research bot", ["research"]);
const claude = model("Claude Sonnet");
const models = [sol, research, claude];

describe("mentionsIn", () => {
  it("finds mentions, ignoring e-mail addresses and trailing punctuation", () => {
    expect(
      mentionsIn("@review what do you think? cc a@b.com and @Claude.").map(
        (m) => m.alias,
      ),
    ).toEqual(["review", "claude"]);
  });

  it("knows where each mention is", () => {
    const [m] = mentionsIn("Hi @rev there");
    expect(m).toEqual({ alias: "rev", start: 3, end: 7 });
    expect(mentionAt("Hi @rev", 7)?.alias).toBe("rev");
    expect(mentionAt("Hi @rev there", 13)).toBeUndefined();
  });
});

describe("resolveAlias", () => {
  it("matches the automatic alias of every model", () => {
    expect(automaticAlias("GPT-6-sol")).toBe("gpt6sol");
    expect(resolveAlias("claudesonnet", models)).toMatchObject({
      kind: "model",
      model: claude,
    });
  });

  it("prefers an exact alias over a longer one with the same start", () => {
    // "r" is exactly GPT-6-sol's alias even though "research" starts with r.
    expect(resolveAlias("r", models)).toMatchObject({ model: sol });
  });

  it("accepts a prefix only when it names one model", () => {
    expect(resolveAlias("rev", models)).toMatchObject({ model: sol });
    expect(resolveAlias("cl", models)).toMatchObject({ model: claude });
    expect(resolveAlias("re", models)).toMatchObject({ kind: "ambiguous" });
    expect(resolveAlias("zz", models)).toEqual({
      kind: "unknown",
      alias: "zz",
    });
  });
});

describe("resolveMentions", () => {
  it("lists each mentioned model once, in order", () => {
    const result = resolveMentions("@cl and @review, then @r again", models);
    expect(result.models).toEqual([claude, sol]);
    expect(result.problems).toEqual([]);
  });

  it("reports mentions that do not name exactly one model", () => {
    const result = resolveMentions("@re and @nobody", models);
    expect(result.problems.map((p) => p.kind)).toEqual([
      "ambiguous",
      "unknown",
    ]);
  });
});
