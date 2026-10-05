// Regression tests for Stage 4: several providers, local models, and
// several models answering in one discussion.

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
  waitFor,
} from "./test/app";

afterEach(cleanup);

async function openModelsTab() {
  await fireEvent.click(screen.getByRole("button", { name: "Settings" }));
  await fireEvent.click(screen.getByRole("tab", { name: "Models" }));
  await screen.findByRole("region", { name: "OpenAI" });
}

function section(name: string) {
  return within(screen.getByRole("region", { name }));
}

function enable(providerId: string, model: string, displayName: string) {
  backend.handle("set_model", {
    providerId,
    model,
    displayName,
    enabled: true,
  });
}

/** GPT-5.6 (OpenAI) and Claude Opus 5.5 (Anthropic), both with keys. */
function twoProviders() {
  backend.keys.set(backend.openai.id, "sk-openai-1111");
  backend.keys.set(backend.anthropic.id, "sk-ant-2222");
  enable(backend.openai.id, "gpt-5.6", "GPT-5.6");
  enable(backend.anthropic.id, "claude-opus-5-5", "Claude Opus 5.5");
}

describe("connecting providers", () => {
  it("sets up Anthropic alongside OpenAI with readable model names", async () => {
    fresh();
    backend.availableModels = ["claude-opus-5-5", "claude-sonnet-5-5"];
    await launch();
    await openModelsTab();
    const anthropic = section("Anthropic");
    expect(anthropic.getByText("No key yet")).toBeTruthy();

    await fireEvent.input(anthropic.getByLabelText("Anthropic API key"), {
      target: { value: "sk-ant-secret-9876" },
    });
    await fireEvent.click(anthropic.getByRole("button", { name: "Save key" }));
    await section("Anthropic").findByText(/ending in 9876/);
    expect(backend.keys.get(backend.anthropic.id)).toBe("sk-ant-secret-9876");
    expect(backend.keys.has(backend.openai.id)).toBe(false);

    await fireEvent.click(
      section("Anthropic").getByRole("button", { name: "Test connection" }),
    );
    await section("Anthropic").findByText(/Connected\. 2 text models/);
    await fireEvent.change(section("Anthropic").getByRole("combobox"), {
      target: { value: "claude-opus-5-5" },
    });
    await fireEvent.click(
      section("Anthropic").getByRole("button", { name: "Add model" }),
    );
    const name = await section("Anthropic").findByLabelText(
      "Name shown for claude-opus-5-5",
    );
    expect((name as HTMLInputElement).value).toBe("Claude Opus 5.5");
  });

  it("adds a local OpenAI-compatible server that needs no key", async () => {
    fresh();
    backend.availableModels = ["qwen3:8b"];
    seed("", "Question");
    await launch();
    await openModelsTab();

    await fireEvent.click(
      screen.getByRole("button", {
        name: "Add an OpenAI-compatible connection…",
      }),
    );
    const form = within(
      screen.getByRole("form", { name: "Add an OpenAI-compatible connection" }),
    );
    await fireEvent.click(form.getByRole("button", { name: "Ollama" }));
    await fireEvent.click(form.getByRole("button", { name: "Add connection" }));

    const ollama = within(
      await screen.findByRole("region", { name: "Ollama" }),
    );
    expect(ollama.getByText("No key needed")).toBeTruthy();
    expect(backend.connections.at(-1)!.baseUrl).toBe(
      "http://localhost:11434/v1",
    );

    await fireEvent.click(
      ollama.getByRole("button", { name: "Test connection" }),
    );
    await ollama.findByText(/Connected\. 1 text model available/);
    await fireEvent.change(ollama.getByRole("combobox"), {
      target: { value: "qwen3:8b" },
    });
    await fireEvent.click(ollama.getByRole("button", { name: "Add model" }));
    await ollama.findByLabelText("Name shown for qwen3:8b");

    // Close Settings: the local model can now be asked without any key.
    await fireEvent.click(screen.getByRole("button", { name: "Close" }));
    await openDiscussion("Question");
    expect(screen.getByRole("button", { name: "Ask qwen3:8b" })).toBeTruthy();
  });

  it("removes an unused connection after confirmation", async () => {
    fresh();
    backend.handle("add_endpoint", {
      displayName: "LM Studio",
      baseUrl: "http://localhost:1234/v1",
      key: null,
    });
    await launch();
    await openModelsTab();
    await fireEvent.click(
      section("LM Studio").getByRole("button", { name: "Remove connection" }),
    );
    const dialog = await waitFor(() => {
      const found = screen
        .getAllByRole("dialog")
        .find((d) => within(d).queryByText("Remove LM Studio?"));
      if (!found) throw new Error("no confirmation");
      return found;
    });
    await fireEvent.click(
      within(dialog).getByRole("button", { name: "Remove connection" }),
    );
    await waitFor(() =>
      expect(screen.queryByRole("region", { name: "LM Studio" })).toBeNull(),
    );
  });
});

describe("several models in one discussion (Stage 4 demo)", () => {
  it("GPT and Claude answer independently; Claude then answers my reply to GPT", async () => {
    fresh();
    twoProviders();
    const { posts } = seed("", "Could Datalog replace our mapping engine?");
    const question = posts[0]!.id;
    await launch();
    await openDiscussion("Could Datalog");

    // Ask both, each as its own reply to the question.
    await fireEvent.click(screen.getByText("Ask…"));
    await fireEvent.click(
      within(
        screen.getByRole("group", { name: "Ask a model to reply" }),
      ).getByRole("button", { name: "All of them" }),
    );
    await waitFor(() => expect(backend.posts).toHaveLength(3));
    const [gpt, claude] = backend.posts.slice(1);
    expect([gpt!.parentId, claude!.parentId]).toEqual([question, question]);
    expect([gpt!.authorId, claude!.authorId]).toEqual([
      "model-gpt-5.6",
      "model-claude-opus-5-5",
    ]);
    await backend.streamText(gpt!.id, "Yes, with stratification.");
    await backend.endReply(gpt!.id, "complete");
    await backend.streamText(
      claude!.id,
      "Mostly; overrides are the hard part.",
    );
    await backend.endReply(claude!.id, "complete");

    // Reply to GPT myself, mentioning Claude so Claude answers, not GPT.
    const gptPost = (
      await screen.findByText("Yes, with stratification.", {
        selector: "article p",
      })
    ).closest("article")!;
    await fireEvent.click(
      within(gptPost).getByRole("button", { name: "Reply to GPT-5.6" }),
    );
    await fireEvent.input(composer(), {
      target: { value: "@claude the Oracle version used priorities instead." },
    });
    expect(screen.getByText("Then asks Claude Opus 5.5")).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: "Post" }));

    await waitFor(() => expect(backend.posts).toHaveLength(5));
    const mine = backend.posts[3]!;
    const answer = backend.posts[4]!;
    expect(mine.parentId).toBe(gpt!.id);
    expect(answer.parentId).toBe(mine.id);
    expect(answer.authorId).toBe("model-claude-opus-5-5");
  });
});
