import { describe, expect, it } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { notebook } from "./notebook.svelte";
import { nav } from "./nav.svelte";

function view(id: string) {
  return {
    discussion: {
      id,
      title: id,
      createdAt: 0,
      updatedAt: 0,
      archivedAt: null,
      pinnedAt: null,
    },
    posts: [],
    participants: [],
    draft: null,
  };
}

describe("notebook.open", () => {
  it("shows the last discussion chosen even if an earlier load finishes later", async () => {
    const pending = new Map<string, (v: unknown) => void>();
    clearMocks();
    mockIPC((cmd, args) => {
      if (cmd !== "get_discussion") return null;
      const id = (args as { id: string }).id;
      return new Promise((resolve) => pending.set(id, resolve));
    });

    const openA = notebook.open("a");
    const openB = notebook.open("b");
    await new Promise((r) => setTimeout(r, 0));

    pending.get("b")!(view("b"));
    await openB;
    pending.get("a")!(view("a"));
    await openA;

    expect(nav.selectedId).toBe("b");
    expect(notebook.view?.discussion.id).toBe("b");
  });

  it("reports a failed load without replacing what is shown", async () => {
    clearMocks();
    mockIPC(() => {
      throw "discussion not found";
    });
    const before = notebook.view;
    await notebook.open("missing");
    expect(notebook.notice).toBe("discussion not found");
    expect(notebook.view).toBe(before);
  });
});
