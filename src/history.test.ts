// Regression tests for Stage 2 flows: editing with history, deletion,
// export, backup and restore, and recovery when the database cannot open.

import { afterEach, describe, expect, it } from "vitest";
import { screen, within } from "@testing-library/svelte";
import {
  backend,
  cleanup,
  fireEvent,
  fresh,
  launch,
  mount,
  openDiscussion,
  seed,
  sidebar,
  streamTexts,
  waitFor,
} from "./test/app";

afterEach(cleanup);

function article(text: string) {
  return screen.getByText(text, { selector: "article p" }).closest("article")!;
}

/** Answers the confirmation dialog, which may open on top of Settings. */
async function confirmWith(label: string) {
  const dialog = await waitFor(() => {
    const found = screen
      .getAllByRole("dialog")
      .find((d) => within(d).queryByRole("button", { name: label }));
    if (!found) throw new Error(`no dialog offers "${label}"`);
    return found;
  });
  await fireEvent.click(within(dialog).getByRole("button", { name: label }));
}

async function openSettings() {
  await fireEvent.click(screen.getByRole("button", { name: "Settings" }));
}

async function chooseFromMenu(item: string) {
  await fireEvent.click(screen.getByText("More"));
  await fireEvent.click(screen.getByRole("button", { name: item }));
}

describe("editing", () => {
  it("replaces the text and keeps the earlier version", async () => {
    fresh();
    seed("", "The attraction is replicaton.");
    await launch();
    await openDiscussion("The attraction");

    await fireEvent.click(
      screen.getByRole("button", { name: "Edit this post" }),
    );
    const editor = within(
      screen.getByRole("form", { name: "Edit post" }),
    ).getByRole("textbox");
    expect((editor as HTMLTextAreaElement).value).toBe(
      "The attraction is replicaton.",
    );
    await fireEvent.input(editor, {
      target: { value: "The attraction is replication." },
    });
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await screen.findByText("The attraction is replication.", {
      selector: "article p",
    });
    const post = article("The attraction is replication.");
    expect(within(post).getByText("Edited")).toBeTruthy();

    await fireEvent.click(
      within(post).getByRole("button", { name: "Show details" }),
    );
    const earlier = await within(post).findByRole("region", {
      name: "Earlier versions",
    });
    expect(
      within(earlier).getByText("The attraction is replicaton."),
    ).toBeTruthy();
  });

  it("Escape abandons an edit and E starts one from the keyboard", async () => {
    fresh();
    seed("", "Original words");
    await launch();
    await openDiscussion("Original words");

    await fireEvent.keyDown(document.body, { key: "j" });
    await fireEvent.keyDown(document.activeElement!, { key: "e" });
    const editor = await screen.findByRole("form", { name: "Edit post" });
    const textbox = within(editor).getByRole("textbox");
    await waitFor(() => expect(document.activeElement).toBe(textbox));
    await fireEvent.input(textbox, { target: { value: "Changed my mind" } });
    await fireEvent.keyDown(textbox, { key: "Escape" });

    expect(screen.queryByRole("form", { name: "Edit post" })).toBeNull();
    expect(streamTexts()).toEqual(["Original words"]);
    expect(backend.revisions).toEqual([]);
  });
});

describe("deleting", () => {
  it("a deleted post keeps its place and its replies", async () => {
    fresh();
    const { discussion, posts } = seed("", "Question");
    backend.handle("add_post", {
      discussionId: discussion.id,
      parentId: posts[0]!.id,
      body: "Regrettable answer",
    });
    backend.handle("add_post", {
      discussionId: discussion.id,
      parentId: backend.posts[1]!.id,
      body: "Follow-up",
    });
    await launch();
    await openDiscussion("Question");

    const regrettable = article("Regrettable answer");
    await fireEvent.click(
      within(regrettable).getByRole("button", { name: "Show details" }),
    );
    await fireEvent.click(
      within(regrettable).getByRole("button", { name: "Delete post" }),
    );

    // Cancelling changes nothing.
    await confirmWith("Cancel");
    expect(backend.posts[1]!.deletedAt).toBeNull();

    await fireEvent.click(
      within(regrettable).getByRole("button", { name: "Delete post" }),
    );
    await confirmWith("Delete post");

    await screen.findByText("This post was deleted.");
    const list = screen.getByRole("list", { name: "Posts in order written" });
    expect(within(list).getAllByRole("listitem")).toHaveLength(3);
    expect(
      screen.getByText(/deleted post/, { selector: "button" }),
    ).toBeTruthy();
    expect(screen.queryByText("Regrettable answer")).toBeNull();
  });

  it("deleting a discussion removes it after confirmation", async () => {
    fresh();
    seed("Private thoughts", "secret");
    seed("Keep this", "public");
    await launch();
    await openDiscussion("Private thoughts");

    await chooseFromMenu("Delete discussion…");
    await confirmWith("Delete discussion");

    await waitFor(() =>
      expect(
        sidebar().queryByRole("button", { name: /Private thoughts/ }),
      ).toBeNull(),
    );
    expect(sidebar().getByRole("button", { name: /Keep this/ })).toBeTruthy();
    expect(backend.discussions.map((d) => d.title)).toEqual(["Keep this"]);
  });
});

describe("export", () => {
  it("writes the discussion where the user chose and says where", async () => {
    fresh();
    seed("Datalog", "Could Datalog replace the mapping engine?");
    await launch();
    await openDiscussion("Datalog");

    backend.dialogPath = "/exports/Datalog.nimata.json";
    await chooseFromMenu("Export as JSON…");
    expect((await screen.findByRole("status")).textContent).toContain(
      "Exported to /exports/Datalog.nimata.json",
    );
    expect(backend.exports.get("/exports/Datalog.nimata.json")).toContain(
      "nimata/1",
    );
  });

  it("cancelling the save dialog does nothing", async () => {
    fresh();
    seed("Datalog", "body");
    await launch();
    await openDiscussion("Datalog");
    backend.dialogPath = null;
    await chooseFromMenu("Export as JSON…");
    await waitFor(() =>
      expect(backend.calls.some((c) => c.cmd === "export_discussion")).toBe(
        true,
      ),
    );
    expect(screen.queryByRole("status")).toBeNull();
    expect(backend.exports.size).toBe(0);
  });
});

describe("backup and restore (Stage 2 demo)", () => {
  it("restores the discussions as they were when backed up", async () => {
    fresh();
    seed("", "Should Nimata use CouchDB?");
    await launch();

    await openSettings();
    await fireEvent.click(screen.getByRole("button", { name: "Back up…" }));
    // Shown both in Settings and in the main status bar behind it.
    await screen.findAllByText(/Backed up to \/backups\/nimata-backup.sqlite3/);
    await fireEvent.click(screen.getByRole("button", { name: "Close" }));

    // Write something after the backup, through the app, so it is visible.
    await fireEvent.click(screen.getByRole("button", { name: "New" }));
    await fireEvent.input(screen.getByRole("textbox", { name: "First post" }), {
      target: { value: "Written after the backup" },
    });
    await fireEvent.click(
      screen.getByRole("button", { name: "Start discussion" }),
    );
    await sidebar().findByRole("button", { name: /Written after the backup/ });

    await openSettings();
    await fireEvent.click(
      screen.getByRole("button", { name: "Restore from backup…" }),
    );
    await confirmWith("Choose backup");

    await screen.findAllByText(
      /Restored from \/backups\/nimata-backup.sqlite3/,
    );
    expect(
      sidebar().queryByRole("button", { name: /Written after the backup/ }),
    ).toBeNull();
    expect(
      sidebar().getByRole("button", { name: /Should Nimata use CouchDB/ }),
    ).toBeTruthy();
  });

  it("a file that is not a backup is refused with a reason", async () => {
    fresh();
    seed("Kept", "body");
    await launch();
    await openSettings();
    backend.dialogPath = "/Downloads/holiday.jpg";
    await fireEvent.click(
      screen.getByRole("button", { name: "Restore from backup…" }),
    );
    await confirmWith("Choose backup");

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("not a Nimata database");
    expect(backend.discussions.map((d) => d.title)).toEqual(["Kept"]);
  });

  it("a database that cannot be opened can be replaced by a backup", async () => {
    fresh();
    seed("Saved before the damage", "body");
    backend.handle("backup_database", {});
    backend.discussions = [];
    backend.posts = [];
    backend.openError =
      "cannot open /data/nimata.sqlite3: the database is damaged: database disk image is malformed";

    await mount();
    expect(
      await screen.findByText("Nimata cannot open your discussions"),
    ).toBeTruthy();
    expect(screen.getByText(/database disk image is malformed/)).toBeTruthy();

    await fireEvent.click(
      screen.getByRole("button", { name: "Restore from backup…" }),
    );
    await confirmWith("Choose backup");

    await screen.findByRole("navigation", { name: "Discussions" });
    await sidebar().findByRole("button", { name: /Saved before the damage/ });
    expect(
      screen.queryByText("Nimata cannot open your discussions"),
    ).toBeNull();
  });
});
