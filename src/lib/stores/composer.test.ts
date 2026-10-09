import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Composers, SAVE_DELAY_MS, type SaveDraft } from "./composer.svelte";

describe("Composers", () => {
  let save: ReturnType<typeof vi.fn<SaveDraft>>;
  let composers: Composers;

  beforeEach(() => {
    vi.useFakeTimers();
    save = vi.fn<SaveDraft>().mockResolvedValue(undefined);
    composers = new Composers(save, vi.fn());
  });

  afterEach(() => vi.useRealTimers());

  it("saves once typing pauses, with the latest text and reply target", () => {
    composers.setReplyTo("d1", "p1");
    composers.setDraft("d1", "Half");
    composers.setDraft("d1", "Half a thought");
    expect(save).not.toHaveBeenCalled();

    vi.advanceTimersByTime(SAVE_DELAY_MS);
    expect(save).toHaveBeenCalledExactlyOnceWith(
      "d1",
      "p1",
      "Half a thought",
      [],
      [],
    );
  });

  it("keeps drafts separate per discussion", () => {
    composers.setDraft("d1", "one");
    composers.setDraft("d2", "two");
    expect(composers.get("d1").draft).toBe("one");
    expect(composers.get("d2").draft).toBe("two");
  });

  it("adopts a stored draft only when nothing newer exists in memory", () => {
    const stored = {
      discussionId: "d1",
      parentId: "p1",
      body: "stored",
      updatedAt: 1,
      contextIds: [],
    };
    composers.load("d1", stored);
    expect(composers.get("d1")).toEqual({
      replyTo: "p1",
      draft: "stored",
      context: [],
      attachments: [],
    });

    composers.setDraft("d1", "newer");
    composers.load("d1", stored);
    expect(composers.get("d1").draft).toBe("newer");
  });

  it("flush saves pending changes immediately", () => {
    composers.setDraft("d1", "unsaved");
    composers.flush();
    expect(save).toHaveBeenCalledExactlyOnceWith("d1", null, "unsaved", [], []);
    vi.advanceTimersByTime(SAVE_DELAY_MS);
    expect(save).toHaveBeenCalledOnce();
  });

  it("clearing after posting cancels a pending save", () => {
    composers.setDraft("d1", "posted text");
    composers.clear("d1");
    vi.advanceTimersByTime(SAVE_DELAY_MS);
    expect(save).not.toHaveBeenCalled();
    expect(composers.get("d1")).toEqual({
      replyTo: null,
      draft: "",
      context: [],
      attachments: [],
    });
  });

  it("keeps files with the draft and saves at once when one is removed", async () => {
    const file = (name: string) => ({
      filename: name,
      mediaType: "text/markdown",
      size: 3,
      contentHash: `sha256:${name}`,
      kind: "text" as const,
    });
    composers.addAttachment("d1", file("a.md"));
    composers.addAttachment("d1", file("b.md"));
    const removed = await composers.removeAttachment("d1", 0);
    expect(removed?.filename).toBe("a.md");
    // Saved without waiting for the typing pause, so the file can be deleted.
    expect(save).toHaveBeenCalledExactlyOnceWith(
      "d1",
      null,
      "",
      [],
      [file("b.md")],
    );
  });
});
