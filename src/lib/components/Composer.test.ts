import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/svelte";
import type { Post } from "../types";

vi.mock("../api", () => ({
  saveDraft: vi.fn().mockResolvedValue(undefined),
  errorMessage: (e: unknown) => String(e),
}));

const { default: Composer } = await import("./Composer.svelte");
const { notebook } = await import("../stores/notebook.svelte");

const parent: Post = {
  id: "p1",
  discussionId: "d1",
  parentId: null,
  authorId: "me",
  body: "The attraction is replication.",
  createdAt: 0,
  tzOffsetMinutes: 0,
  editedAt: null,
  status: "complete",
};

function renderComposer() {
  render(Composer, {
    discussionId: "d1",
    postsById: new Map([[parent.id, parent]]),
    nameOf: () => "Thanos",
    onPosted: vi.fn(),
  });
}

describe("Composer", () => {
  it("shows the reply target and clears it with Escape", async () => {
    notebook.composers.setReplyTo("d1", "p1");
    renderComposer();
    expect(screen.getByText(/The attraction is replication/)).toBeTruthy();

    await fireEvent.keyDown(screen.getByRole("textbox"), { key: "Escape" });
    expect(notebook.composers.get("d1").replyTo).toBeNull();
    expect(screen.getByText("New thread")).toBeTruthy();
  });

  it("enables posting only when there is text", async () => {
    notebook.composers.clear("d1");
    renderComposer();
    const post = screen.getByRole("button", { name: "Post" });
    expect(post.hasAttribute("disabled")).toBe(true);

    await fireEvent.input(screen.getByRole("textbox"), {
      target: { value: "But mobile changes the constraints." },
    });
    expect(post.hasAttribute("disabled")).toBe(false);
    expect(notebook.composers.get("d1").draft).toBe(
      "But mobile changes the constraints.",
    );
  });
});
