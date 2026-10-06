import { describe, expect, it } from "vitest";
import { defaultTarget, whoAnswers } from "./compose";
import type { ModelParticipant, Post } from "./types";

function model(id: string, aliases: string[] = []): ModelParticipant {
  return {
    participant: {
      id,
      kind: "model",
      displayName: id,
      provider: "openai",
      model: id,
    },
    providerId: "openai",
    enabled: true,
    aliases,
  };
}

function post(
  id: string,
  authorId: string,
  createdAt: number,
  extra: Partial<Post> = {},
): Post {
  return {
    id,
    discussionId: "d",
    parentId: null,
    authorId,
    body: id,
    createdAt,
    tzOffsetMinutes: 0,
    editedAt: null,
    deletedAt: null,
    status: "complete",
    providerMetadata: null,
    contextIds: [],
    ...extra,
  };
}

const gpt = model("gpt", ["review"]);
const claude = model("claude");
const askable = [gpt, claude];

describe("defaultTarget", () => {
  it("is the newest post that was not deleted and did not fail", () => {
    const posts = [
      post("a", "me", 1),
      post("b", "me", 2),
      post("c", "gpt", 3, { status: "failed" }),
      post("d", "me", 4, { deletedAt: 5 }),
    ];
    expect(defaultTarget(posts)?.id).toBe("b");
    expect(
      defaultTarget([...posts, post("e", "gpt", 6, { status: "streaming" })])
        ?.id,
    ).toBe("e");
    expect(defaultTarget([])).toBeUndefined();
  });
});

describe("whoAnswers", () => {
  const base = {
    text: "",
    choice: null,
    posts: [] as Post[],
    askable,
    defaultModelId: null,
  };

  it("asks the mentioned models before anything else", () => {
    const target = post("a", "claude", 1);
    const result = whoAnswers({
      ...base,
      text: "@review thoughts?",
      target,
      choice: "claude",
    });
    expect(result.models).toEqual([gpt]);
    expect(result.reason).toBe("mentioned");
  });

  it("asks the model being replied to", () => {
    const target = post("a", "claude", 1);
    expect(whoAnswers({ ...base, target }).models).toEqual([claude]);
  });

  it("otherwise asks the model that last replied in the discussion", () => {
    const posts = [post("a", "me", 1), post("b", "gpt", 2), post("c", "me", 3)];
    const result = whoAnswers({ ...base, posts, target: posts[2] });
    expect(result.models).toEqual([gpt]);
    expect(result.reason).toBe("last in thread");
  });

  it("falls back to the default model, then to no one", () => {
    const target = post("a", "me", 1);
    expect(
      whoAnswers({ ...base, target, defaultModelId: "claude" }).models,
    ).toEqual([claude]);
    expect(whoAnswers({ ...base, target }).reason).toBe("nobody");
  });

  it("an explicit choice wins over the thread, and No one asks nobody", () => {
    const target = post("a", "gpt", 1);
    expect(whoAnswers({ ...base, target, choice: "claude" }).models).toEqual([
      claude,
    ]);
    expect(whoAnswers({ ...base, target, choice: "none" }).models).toEqual([]);
  });

  it("ignores models that cannot be asked", () => {
    const target = post("a", "retired-model", 1);
    expect(whoAnswers({ ...base, target }).reason).toBe("nobody");
  });

  it("a mention problem asks nobody", () => {
    const result = whoAnswers({ ...base, text: "@nobody", target: undefined });
    expect(result.models).toEqual([]);
    expect(result.mentions.problems).toHaveLength(1);
  });
});
