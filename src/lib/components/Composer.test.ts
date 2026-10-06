import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/svelte";
import type { Post } from "../types";

vi.mock("../api", () => ({
  saveDraft: vi.fn().mockResolvedValue(undefined),
  errorMessage: (e: unknown) => String(e),
}));

const { default: Composer } = await import("./Composer.svelte");
const { notebook } = await import("../stores/notebook.svelte");

function post(id: string, body: string, createdAt: number): Post {
  return {
    id,
    discussionId: "d1",
    parentId: null,
    authorId: "me",
    body,
    createdAt,
    tzOffsetMinutes: 0,
    editedAt: null,
    deletedAt: null,
    status: "complete",
    providerMetadata: null,
    contextIds: [],
  };
}

const first = post("p1", "The attraction is replication.", 1);
const latest = post("p2", "But mobile changes the constraints.", 2);

function renderComposer() {
  const posts = [first, latest];
  render(Composer, {
    discussionId: "d1",
    posts,
    postsById: new Map(posts.map((p) => [p.id, p])),
    nameOf: () => "Thanos",
    onPosted: vi.fn(),
  });
}

describe("Composer", () => {
  it("replies to the latest post unless another is chosen", async () => {
    notebook.composers.clear("d1");
    renderComposer();
    expect(screen.getByText(/But mobile changes the constraints/)).toBeTruthy();
    expect(
      screen.queryByRole("button", { name: "Reply to latest" }),
    ).toBeNull();

    notebook.composers.setReplyTo("d1", "p1");
    await screen.findByText(/The attraction is replication/);

    await fireEvent.keyDown(screen.getByRole("textbox"), { key: "Escape" });
    expect(notebook.composers.get("d1").replyTo).toBeNull();
    await screen.findByText(/But mobile changes the constraints/);
  });

  it("enables posting only when there is text", async () => {
    notebook.composers.clear("d1");
    renderComposer();
    const button = screen.getByRole("button", { name: "Post" });
    expect(button.hasAttribute("disabled")).toBe(true);

    await fireEvent.input(screen.getByRole("textbox"), {
      target: { value: "A reply" },
    });
    expect(button.hasAttribute("disabled")).toBe(false);
    expect(notebook.composers.get("d1").draft).toBe("A reply");
  });
});
