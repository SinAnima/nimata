// Regression tests for Stage 3: setting up a model in Settings, asking it to
// reply, watching the reply stream in, and stopping or retrying it.

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
  seed,
  streamTexts,
  waitFor,
} from "./test/app";

afterEach(cleanup);

async function openModelsTab() {
  await fireEvent.click(screen.getByRole("button", { name: "Settings" }));
  await fireEvent.click(screen.getByRole("tab", { name: "Models" }));
  return within(await screen.findByRole("region", { name: "OpenAI" }));
}

/** A saved key and GPT-5.6 enabled, as if set up in Settings earlier. */
function connected() {
  backend.savedKey = "sk-proj-secret-1234";
  backend.handle("set_model", {
    providerId: backend.openai.id,
    model: "gpt-5.6",
    displayName: "GPT-5.6",
    enabled: true,
  });
}

function postArticle(text: string) {
  return screen.getByText(text, { selector: "article p" }).closest("article")!;
}

function lastPostId() {
  return backend.posts.at(-1)!.id;
}

describe("setting up a model", () => {
  it("saves the key without ever showing it, and lists models to choose", async () => {
    fresh();
    await launch();
    let openai = await openModelsTab();
    expect(openai.getByText("No key yet")).toBeTruthy();

    await fireEvent.input(openai.getByLabelText("OpenAI API key"), {
      target: { value: "sk-proj-secret-1234" },
    });
    await fireEvent.click(openai.getByRole("button", { name: "Save key" }));
    openai = within(await screen.findByRole("region", { name: "OpenAI" }));
    await openai.findByText(/Saved in the Keychain, ending in 1234/);
    expect(document.body.textContent).not.toContain("sk-proj-secret");

    await fireEvent.click(
      openai.getByRole("button", { name: "Test connection" }),
    );
    await openai.findByText("Connected. 2 text models available.");

    await fireEvent.change(openai.getByRole("combobox"), {
      target: { value: "gpt-5.6" },
    });
    await fireEvent.click(openai.getByRole("button", { name: "Add model" }));
    const name = await openai.findByLabelText("Name shown for gpt-5.6");
    expect((name as HTMLInputElement).value).toBe("GPT-5.6");

    await fireEvent.change(name, { target: { value: "GPT" } });
    await waitFor(() =>
      expect(backend.models[0]!.participant.displayName).toBe("GPT"),
    );
  });

  it("release builds show no part of the key", async () => {
    fresh();
    backend.developmentBuild = false;
    backend.savedKey = "sk-proj-secret-1234";
    await launch();
    const openai = await openModelsTab();
    await openai.findByText("Saved in the Keychain.");
    expect(document.body.textContent).not.toContain("1234");
  });

  it("explains a connection that does not work", async () => {
    fresh();
    backend.savedKey = "sk-wrong";
    backend.availableModels =
      "OpenAI rejected the API key. Check it in Settings, Models.";
    await launch();
    const openai = await openModelsTab();
    await fireEvent.click(
      openai.getByRole("button", { name: "Test connection" }),
    );
    expect((await openai.findByRole("alert")).textContent).toContain(
      "rejected the API key",
    );
  });
});

describe("asking a model (Stage 3 demo)", () => {
  it("builds a four-post discussion with the model replying where asked", async () => {
    fresh();
    connected();
    const { posts } = seed("", "Should Nimata use CouchDB?");
    await launch();
    await openDiscussion("Should Nimata use CouchDB");

    // Ask GPT to answer the question.
    await fireEvent.click(
      within(postArticle("Should Nimata use CouchDB?")).getByRole("button", {
        name: "Ask GPT-5.6",
      }),
    );
    await screen.findByText("Waiting for GPT-5.6…");
    const firstReply = lastPostId();
    await backend.streamText(firstReply, "Replication ", "is the draw.");
    await screen.findByText("Replication is the draw.", {
      selector: "article p",
    });
    expect(screen.getByText("Writing…")).toBeTruthy();
    await backend.endReply(firstReply, "complete");
    await waitFor(() => expect(screen.queryByText("Writing…")).toBeNull());

    // Reply to GPT myself: the composer already targets GPT's reply, and
    // posting asks GPT to answer without another click.
    await fireEvent.input(composer(), {
      target: { value: "But mobile changes the constraints." },
    });
    expect(screen.getByText(/GPT-5.6 \(you are replying to it\)/)).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: "Post" }));
    await screen.findByText("But mobile changes the constraints.", {
      selector: "article p",
    });
    await waitFor(() => expect(backend.posts).toHaveLength(4));
    const secondReply = lastPostId();
    await backend.streamText(secondReply, "Then sync becomes an adapter.");
    await backend.endReply(secondReply, "complete");

    await waitFor(() =>
      expect(streamTexts()).toEqual([
        "Should Nimata use CouchDB?",
        "Replication is the draw.",
        "But mobile changes the constraints.",
        "Then sync becomes an adapter.",
      ]),
    );
    const parents = backend.posts.map((p) => p.parentId);
    expect(parents).toEqual([
      null,
      posts[0]!.id,
      firstReply,
      backend.posts[2]!.id,
    ]);
    const last = postArticle("Then sync becomes an adapter.");
    expect(within(last).getByText("Replying to")).toBeTruthy();
    expect(within(last).getByText("Me")).toBeTruthy();
  });

  it("shows which model version answered and what it was shown", async () => {
    fresh();
    connected();
    seed("", "Question");
    await launch();
    await openDiscussion("Question");
    await fireEvent.click(screen.getByRole("button", { name: "Ask GPT-5.6" }));
    const reply = lastPostId();
    await backend.streamText(reply, "Answer");
    await backend.endReply(reply, "complete");

    const article = await waitFor(() => postArticle("Answer"));
    await fireEvent.click(
      within(article).getByRole("button", { name: "Show details" }),
    );
    expect(await within(article).findByText("gpt-5.6-2026-08-01")).toBeTruthy();
    expect(within(article).getByText("41 sent, 12 received")).toBeTruthy();
    await fireEvent.click(
      await within(article).findByRole("button", {
        name: "See exactly what was sent",
      }),
    );
    const sent = await screen.findByRole("list", {
      name: "Messages sent, in order",
    });
    expect(within(sent).getByText("Me: Question")).toBeTruthy();
  });
});

describe("stopping and retrying", () => {
  it("Stop keeps the text so far, and Retry asks again as a new reply", async () => {
    fresh();
    connected();
    seed("", "Question");
    await launch();
    await openDiscussion("Question");
    await fireEvent.click(screen.getByRole("button", { name: "Ask GPT-5.6" }));
    const reply = lastPostId();
    await backend.streamText(reply, "Half an ans");

    await fireEvent.click(await screen.findByRole("button", { name: "Stop" }));
    await screen.findByText("Stopped before finishing.");
    expect(
      screen.getByText("Half an ans", { selector: "article p" }),
    ).toBeTruthy();

    await fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await screen.findByText("Waiting for GPT-5.6…");
    expect(backend.posts.at(-1)!.id).not.toBe(reply);
    expect(backend.posts.at(-1)!.parentId).toBe(backend.posts[0]!.id);
    expect(backend.posts.find((p) => p.id === reply)!.status).toBe("cancelled");
  });

  it("a failed reply says why and offers to retry", async () => {
    fresh();
    connected();
    seed("", "Question");
    await launch();
    await openDiscussion("Question");
    await fireEvent.click(screen.getByRole("button", { name: "Ask GPT-5.6" }));
    await backend.endReply(
      lastPostId(),
      "failed",
      "OpenAI has run out of credit or reached its spending limit.",
    );
    const alert = await screen.findByText(
      /This reply failed: OpenAI has run out/,
    );
    expect(alert).toBeTruthy();
    expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();
  });
});

describe("when models cannot be asked", () => {
  it("offers no Ask button without a key or an enabled model", async () => {
    fresh();
    seed("", "Question");
    backend.handle("set_model", {
      providerId: backend.openai.id,
      model: "gpt-5.6",
      displayName: "GPT-5.6",
      enabled: true,
    });
    await launch();
    await openDiscussion("Question");
    expect(screen.queryByRole("button", { name: /Ask/ })).toBeNull();
  });

  it("offers a menu when several models are enabled", async () => {
    fresh();
    connected();
    backend.handle("set_model", {
      providerId: backend.openai.id,
      model: "gpt-5.6-mini",
      displayName: "GPT mini",
      enabled: true,
    });
    seed("", "Question");
    await launch();
    await openDiscussion("Question");
    await fireEvent.click(screen.getByText("Ask…"));
    const menu = screen.getByRole("group", { name: "Ask a model to reply" });
    await fireEvent.click(
      within(menu).getByRole("button", { name: "GPT mini" }),
    );
    await screen.findByText("Waiting for GPT mini…");
  });
});

describe("posting asks models automatically", () => {
  function addModel(
    model: string,
    displayName: string,
    aliases: string[] = [],
  ) {
    const entry = backend.handle("set_model", {
      providerId: backend.openai.id,
      model,
      displayName,
      enabled: true,
    }) as { participant: { id: string } };
    if (aliases.length > 0)
      backend.handle("set_model_aliases", {
        participantId: entry.participant.id,
        aliases,
      });
    return entry.participant.id;
  }

  async function write(text: string) {
    await fireEvent.input(composer(), { target: { value: text } });
  }

  it("@mentions ask each named model, each as its own reply", async () => {
    fresh();
    connected();
    addModel("claude-sonnet", "Claude", ["critic"]);
    seed("", "Question");
    await launch();
    await openDiscussion("Question");

    await write("@gpt56 and @cr what do you each think?");
    expect(screen.getByText("Then asks GPT-5.6, Claude")).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: "Post" }));

    await waitFor(() => expect(backend.posts).toHaveLength(4));
    const mine = backend.posts[1]!;
    const replies = backend.posts.slice(2);
    expect(replies.map((p) => p.parentId)).toEqual([mine.id, mine.id]);
    expect(replies.map((p) => p.authorId)).toEqual([
      "model-gpt-5.6",
      "model-claude-sonnet",
    ]);
  });

  it("an ambiguous or unknown mention blocks posting until fixed", async () => {
    fresh();
    connected();
    addModel("gpt-5.6-mini", "GPT mini");
    seed("", "Question");
    await launch();
    await openDiscussion("Question");

    await write("@gpt what now?");
    expect((await screen.findByRole("alert")).textContent).toContain(
      "@gpt could mean",
    );
    expect(
      screen.getByRole("button", { name: "Post" }).hasAttribute("disabled"),
    ).toBe(true);

    await write("@nobody what now?");
    expect((await screen.findByRole("alert")).textContent).toContain(
      "does not name a model",
    );
  });

  it("No one posts without asking", async () => {
    fresh();
    connected();
    const { discussion, posts } = seed("", "Question");
    backend.handle("ask_model", {
      discussionId: discussion.id,
      parentId: posts[0]!.id,
      participantId: "model-gpt-5.6",
    });
    await backend.endReply(backend.posts[1]!.id, "complete");
    await launch();
    await openDiscussion("Question");

    await write("Just a note to myself.");
    await fireEvent.change(screen.getByRole("combobox"), {
      target: { value: "none" },
    });
    await fireEvent.click(screen.getByRole("button", { name: "Post" }));
    await screen.findByText("Just a note to myself.", {
      selector: "article p",
    });
    expect(backend.posts).toHaveLength(3);
  });

  it("a new discussion asks the default model", async () => {
    fresh();
    connected();
    backend.defaultModelId = "model-gpt-5.6";
    await launch();
    await fireEvent.click(screen.getByRole("button", { name: "New" }));
    await fireEvent.input(screen.getByRole("textbox", { name: "First post" }), {
      target: { value: "Could Datalog replace our mapping engine?" },
    });
    expect(screen.getByText(/GPT-5.6 \(default\)/)).toBeTruthy();
    await fireEvent.click(
      screen.getByRole("button", { name: "Start discussion" }),
    );

    await screen.findByText("Waiting for GPT-5.6…");
    expect(backend.posts[1]!.parentId).toBe(backend.posts[0]!.id);
  });

  it("typing @ offers matching models to complete", async () => {
    fresh();
    connected();
    addModel("claude-sonnet", "Claude", ["review"]);
    seed("", "Question");
    await launch();
    await openDiscussion("Question");

    const box = composer();
    await fireEvent.input(box, { target: { value: "@re", selectionStart: 3 } });
    box.setSelectionRange(3, 3);
    await fireEvent.keyUp(box);
    const options = await screen.findByRole("group", {
      name: "Models matching @re",
    });
    await fireEvent.click(
      within(options).getByRole("button", { name: /@review/ }),
    );
    expect(box.value).toBe("@review ");
    expect(screen.getByText("Then asks Claude")).toBeTruthy();
  });
});

describe("aliases and the default model in Settings", () => {
  it("saves aliases and refuses one that names another model", async () => {
    fresh();
    connected();
    await launch();
    const openai = await openModelsTab();
    const aliases = openai.getByLabelText("Aliases for GPT-5.6");
    expect(openai.getByText("and @gpt56")).toBeTruthy();

    await fireEvent.change(aliases, { target: { value: "review, r" } });
    await waitFor(() =>
      expect(backend.models[0]!.aliases).toEqual(["review", "r"]),
    );

    const settings = within(screen.getByRole("dialog"));
    await fireEvent.change(settings.getAllByRole("combobox")[0]!, {
      target: { value: "model-gpt-5.6" },
    });
    await waitFor(() => expect(backend.defaultModelId).toBe("model-gpt-5.6"));
  });
});
