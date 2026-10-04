import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/svelte";
import PostItem from "./PostItem.svelte";
import type { Participant, Post } from "../types";

const claude: Participant = {
  id: "p-claude",
  kind: "model",
  displayName: "Claude Sonnet",
  provider: "anthropic",
  model: "claude-sonnet",
};

function post(id: string, body: string, parentId: string | null): Post {
  return {
    id,
    discussionId: "d",
    parentId,
    authorId: claude.id,
    body,
    createdAt: Date.UTC(2026, 9, 4, 16, 41),
    tzOffsetMinutes: -240,
    editedAt: null,
    status: "complete",
  };
}

const parent = post(
  "parent",
  "The Oracle implementation used priorities.",
  null,
);
const reply = post("reply", "That changes the answer.", "parent");

function renderReply(quoteParent: boolean, onShowParent = vi.fn()) {
  render(PostItem, {
    post: reply,
    author: claude,
    parent,
    parentAuthorName: "Thanos",
    quoteParent,
    now: Date.UTC(2026, 9, 4, 18, 0),
    flashing: false,
    isReplyTarget: false,
    onReply: vi.fn(),
    onShowParent,
  });
  return onShowParent;
}

describe("PostItem", () => {
  it("names the parent's author and reveals the parent when activated", async () => {
    const onShowParent = renderReply(false);
    const ref = screen.getByRole("button", { name: /Replying to Thanos/ });
    await fireEvent.click(ref);
    expect(onShowParent).toHaveBeenCalledOnce();
  });

  it("quotes the parent only when asked to", () => {
    renderReply(true);
    expect(screen.getByText(parent.body)).toBeTruthy();
  });

  it("shows the author's exact wall-clock time on request", async () => {
    renderReply(false);
    await fireEvent.click(
      screen.getByRole("button", { name: "Show exact time" }),
    );
    expect(screen.getByText("2026-10-04 12:41:00 UTC-04:00")).toBeTruthy();
  });
});
