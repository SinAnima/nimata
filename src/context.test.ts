// Regression tests for Stage 5: choosing context from other branches,
// previewing what a model will be sent, and seeing what it was sent.

import { afterEach, describe, expect, it } from "vitest";
import { screen, within } from "@testing-library/svelte";
import {
  backend,
  cleanup,
  composer,
  fireEvent,
  fresh,
  launch,
  openDiscussion,
  restart,
  seed,
  waitFor,
} from "./test/app";

afterEach(cleanup);

function article(text: string) {
  return screen.getByText(text, { selector: "article p" }).closest("article")!;
}

/** A question with GPT and Claude answering on sibling branches. */
async function twoBranches() {
  fresh();
  backend.keys.set(backend.openai.id, "sk-1");
  backend.keys.set(backend.anthropic.id, "sk-2");
  for (const [providerId, model, displayName] of [
    [backend.openai.id, "gpt-5.6", "GPT-5.6"],
    [backend.anthropic.id, "claude-opus-5-5", "Claude"],
    [backend.openai.id, "grok-5", "Grok"],
  ])
    backend.handle("set_model", {
      providerId,
      model,
      displayName,
      enabled: true,
    });
  const { discussion, posts } = seed("", "Should Nimata use CouchDB?");
  for (const [participantId, body] of [
    ["model-gpt-5.6", "Yes: replication is the draw."],
    ["model-claude-opus-5-5", "No: mobile changes the constraints."],
  ]) {
    backend.handle("ask_model", {
      discussionId: discussion.id,
      parentId: posts[0]!.id,
      participantId,
    });
    const reply = backend.posts.at(-1)!;
    await backend.streamText(reply.id, body!);
    await backend.endReply(reply.id, "complete");
  }
  await launch();
  await openDiscussion("Should Nimata use CouchDB");
}

describe("including context (Stage 5 demo)", () => {
  it("replies to GPT while including Claude's branch, and shows exactly what Grok gets", async () => {
    await twoBranches();
    const gpt = article("Yes: replication is the draw.");
    const claude = article("No: mobile changes the constraints.");

    // Reply to GPT, including Claude's argument as context.
    await fireEvent.click(
      within(gpt).getByRole("button", { name: "Reply to GPT-5.6" }),
    );
    await fireEvent.click(
      within(claude).getByRole("button", { name: "Include" }),
    );
    expect(
      within(claude).getByRole("button", { name: "Included" }),
    ).toBeTruthy();
    const chips = screen.getByRole("group", { name: "Also considering" });
    expect(within(chips).getByText("Claude")).toBeTruthy();

    // Ask Grok rather than GPT, and preview what it will receive.
    await fireEvent.input(composer(), {
      target: { value: "@grok weigh both arguments." },
    });
    await fireEvent.click(
      screen.getByRole("button", { name: "What will be sent?" }),
    );
    const preview = await screen.findByRole("list", {
      name: "Messages sent, in order",
    });
    const labels = within(preview)
      .getAllByRole("listitem")
      .map((li) => li.querySelector("p")!.textContent);
    expect(labels).toEqual([
      "Reply chain",
      "Reply chain",
      "Context from another branch",
      "Reply chain",
    ]);
    expect(
      within(preview).getByText(
        /Claude wrote:\s+No: mobile changes the constraints\./,
      ),
    ).toBeTruthy();
    await fireEvent.click(
      screen.getAllByRole("button", { name: "Close" }).at(-1)!,
    );

    // Post it: the reference is kept, the chips clear, and Grok is asked.
    await fireEvent.click(screen.getByRole("button", { name: "Post" }));
    await waitFor(() => expect(backend.posts).toHaveLength(5));
    const mine = backend.posts[3]!;
    expect(mine.parentId).toBe(backend.posts[1]!.id);
    expect(mine.contextIds).toEqual([backend.posts[2]!.id]);
    expect(
      screen.queryByRole("group", { name: "Also considering" }),
    ).toBeNull();
    expect(
      within(article("@grok weigh both arguments.")).getByText(
        /Also considering/,
      ),
    ).toBeTruthy();

    const grok = backend.posts[4]!;
    expect(grok.authorId).toBe("model-grok-5");
    const sent = backend.generations.find((g) => g.postId === grok.id)!.sent!;
    expect(sent.messages.map((m) => m.source)).toEqual([
      "thread",
      "thread",
      "context",
      "thread",
    ]);
  });

  it("chosen context survives a restart with the draft", async () => {
    await twoBranches();
    await fireEvent.click(
      within(article("No: mobile changes the constraints.")).getByRole(
        "button",
        {
          name: "Include",
        },
      ),
    );
    await fireEvent.input(composer(), { target: { value: "Thinking…" } });
    await restart();
    await openDiscussion("Should Nimata use CouchDB");
    const chips = await screen.findByRole("group", {
      name: "Also considering",
    });
    expect(within(chips).getByText("Claude")).toBeTruthy();
  });

  it("a chip removes its post from the context", async () => {
    await twoBranches();
    const claude = article("No: mobile changes the constraints.");
    await fireEvent.click(
      within(claude).getByRole("button", { name: "Include" }),
    );
    await fireEvent.click(
      screen.getByRole("button", { name: "Stop considering Claude's post" }),
    );
    expect(
      screen.queryByRole("group", { name: "Also considering" }),
    ).toBeNull();
    expect(
      within(claude).getByRole("button", { name: "Include" }),
    ).toBeTruthy();
  });

  it("C includes the focused post", async () => {
    await twoBranches();
    await fireEvent.keyDown(document.body, { key: "j" });
    await fireEvent.keyDown(document.body, { key: "j" });
    await fireEvent.keyDown(document.activeElement!, { key: "c" });
    const chips = await screen.findByRole("group", {
      name: "Also considering",
    });
    expect(within(chips).getByText("GPT-5.6")).toBeTruthy();
  });
});
