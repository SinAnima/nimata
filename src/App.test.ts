// Regression tests for whole user flows: the real App, stores, and api.ts
// running against an in-memory backend through Tauri's IPC mock.

import { afterEach, describe, expect, it } from "vitest";
import { screen, within } from "@testing-library/svelte";
import {
  backend,
  cleanup,
  composer,
  fireEvent,
  fresh,
  launch,
  mount,
  openDiscussion,
  restart,
  seed,
  sidebar,
  streamTexts,
  waitFor,
  writeAndPost,
} from "./test/app";

afterEach(cleanup);

describe("first run", () => {
  it("shows an empty notebook and starts a discussion with a derived title", async () => {
    fresh();
    await launch();
    expect(await screen.findByText("No discussions yet.")).toBeTruthy();

    await fireEvent.click(
      screen.getByRole("button", { name: "Start a discussion" }),
    );
    const body = screen.getByRole("textbox", { name: "First post" });
    await fireEvent.input(body, {
      target: { value: "Should Nimata use CouchDB?" },
    });
    await fireEvent.click(
      screen.getByRole("button", { name: "Start discussion" }),
    );

    await screen.findByText("Should Nimata use CouchDB?", {
      selector: "article p",
    });
    await sidebar().findByRole("button", { name: /Should Nimata use CouchDB/ });
    expect(backend.discussions).toHaveLength(1);
  });

  it("cannot start a discussion without a first post", async () => {
    fresh();
    await launch();
    await fireEvent.click(screen.getByRole("button", { name: "New" }));
    const start = screen.getByRole("button", { name: "Start discussion" });
    expect(start.hasAttribute("disabled")).toBe(true);
  });
});

describe("threaded replies", () => {
  it("replies to any post and reads in the order written (Stage 1 demo)", async () => {
    fresh();
    const { posts } = seed("", "Should Nimata use CouchDB?");
    const rootId = posts[0]!.id;
    await launch();
    await openDiscussion("Should Nimata use CouchDB");

    const replyButtons = () =>
      screen.getAllByRole("button", { name: "Reply to Me" });
    await fireEvent.click(replyButtons()[0]!);
    await writeAndPost("The attraction is replication.");
    await fireEvent.click(replyButtons()[1]!);
    await writeAndPost("But mobile changes the constraints.");
    await fireEvent.click(replyButtons()[0]!);
    await writeAndPost("A second branch from the question.");

    expect(streamTexts()).toEqual([
      "Should Nimata use CouchDB?",
      "The attraction is replication.",
      "But mobile changes the constraints.",
      "A second branch from the question.",
    ]);
    expect(backend.posts.map((p) => p.parentId)).toEqual([
      null,
      rootId,
      backend.posts[1]!.id,
      rootId,
    ]);

    // The last post's parent is not directly above it, so it is quoted.
    const items = within(
      screen.getByRole("list", { name: "Posts in order written" }),
    ).getAllByRole("listitem");
    const last = items.at(-1)!;
    expect(
      within(last).getByText("Should Nimata use CouchDB?", {
        selector: "span",
      }),
    ).toBeTruthy();
  });

  it("posting clears the composer and its reply target", async () => {
    fresh();
    seed("", "Question");
    await launch();
    await openDiscussion("Question");
    await fireEvent.click(screen.getByRole("button", { name: "Reply to Me" }));
    await writeAndPost("Answer");
    expect(composer().value).toBe("");
    // The next post replies to the newest one.
    expect(
      screen.getByText(/Replying to/, { selector: "form span" }).textContent,
    ).toContain("Answer");
  });
});

describe("persistence across restarts", () => {
  it("keeps the unsent draft and its reply target", async () => {
    fresh();
    const { posts } = seed("", "Should Nimata use CouchDB?");
    await launch();
    await openDiscussion("Should Nimata use CouchDB");
    await fireEvent.click(screen.getByRole("button", { name: "Reply to Me" }));
    await fireEvent.input(composer(), { target: { value: "Half a thought" } });

    await restart();
    await openDiscussion("Should Nimata use CouchDB");
    expect(composer().value).toBe("Half a thought");
    expect(
      screen.getByText(/Replying to/, { selector: "form span" }).textContent,
    ).toContain("Should Nimata use CouchDB?");
    expect(backend.drafts.get(backend.discussions[0]!.id)?.parentId).toBe(
      posts[0]!.id,
    );
  });
});

describe("failures", () => {
  it("a failed post keeps the text and explains what happened", async () => {
    fresh();
    seed("", "Question");
    await launch();
    await openDiscussion("Question");
    backend.failNext("add_post", "storage error: disk full");

    await fireEvent.input(composer(), { target: { value: "Important words" } });
    await fireEvent.click(screen.getByRole("button", { name: "Post" }));

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("storage error: disk full");
    expect(composer().value).toBe("Important words");
    await fireEvent.click(
      within(alert).getByRole("button", { name: "Dismiss" }),
    );
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("shows why the database cannot be opened instead of an empty app", async () => {
    fresh();
    backend.failNext(
      "local_user",
      "cannot open /data/nimata.sqlite3: disk I/O error",
    );
    await mount();
    expect(
      await screen.findByText("Nimata cannot open your discussions"),
    ).toBeTruthy();
    expect(screen.getByText(/disk I\/O error/)).toBeTruthy();
  });
});

describe("organising discussions", () => {
  it("renames with Enter and abandons a rename with Escape", async () => {
    fresh();
    seed("Draft title", "Body");
    await launch();
    await openDiscussion("Draft title");

    await fireEvent.click(
      screen.getAllByRole("button", { name: "Draft title" })[0]!,
    );
    let input = screen.getByRole("textbox", { name: "Discussion title" });
    await fireEvent.input(input, { target: { value: "Final title" } });
    await fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() =>
      expect(backend.discussions[0]!.title).toBe("Final title"),
    );
    await sidebar().findByRole("button", { name: /Final title/ });

    await fireEvent.click(
      screen.getAllByRole("button", { name: "Final title" })[0]!,
    );
    input = screen.getByRole("textbox", { name: "Discussion title" });
    await fireEvent.input(input, { target: { value: "Discarded" } });
    await fireEvent.keyDown(input, { key: "Escape" });
    expect(backend.discussions[0]!.title).toBe("Final title");
  });

  it("archives into a separate list and back", async () => {
    fresh();
    seed("Keep for later", "Body");
    await launch();
    await openDiscussion("Keep for later");

    await fireEvent.click(screen.getByRole("button", { name: "Archive" }));
    await screen.findByText("No discussions yet.");

    await fireEvent.click(
      screen.getByRole("button", { name: "Archived discussions" }),
    );
    await openDiscussion("Keep for later");
    await fireEvent.click(screen.getByRole("button", { name: "Unarchive" }));
    await fireEvent.click(
      screen.getByRole("button", { name: "Back to discussions" }),
    );
    await sidebar().findByRole("button", { name: /Keep for later/ });
    expect(backend.discussions[0]!.archivedAt).toBeNull();
  });

  it("orders discussions by their latest post", async () => {
    fresh();
    seed("Older", "first");
    seed("Newer", "second");
    await launch();
    const titles = () =>
      sidebar()
        .queryAllByRole("listitem")
        .map((li) => li.querySelector("span")?.textContent);
    await waitFor(() => expect(titles()).toEqual(["Newer", "Older"]));

    await openDiscussion("Older");
    await writeAndPost("revived");
    await waitFor(() => expect(titles()).toEqual(["Older", "Newer"]));
  });
});

describe("keyboard", () => {
  it("Ctrl+N opens a new discussion and Escape returns", async () => {
    fresh();
    seed("Existing", "Body");
    await launch();
    await openDiscussion("Existing");

    await fireEvent.keyDown(window, { key: "n", ctrlKey: true });
    const body = await screen.findByRole("textbox", { name: "First post" });
    await fireEvent.keyDown(body, { key: "Escape" });
    await screen.findByRole("list", { name: "Posts in order written" });
  });

  it("J and K move between posts and R replies to the focused one", async () => {
    fresh();
    const { discussion, posts } = seed("", "First");
    backend.handle("add_post", {
      discussionId: discussion.id,
      parentId: posts[0]!.id,
      body: "Second",
    });
    const notebook = await launch();
    await openDiscussion("First");

    const focusedText = () =>
      document.activeElement?.querySelector("div.font-serif p")?.textContent;
    await fireEvent.keyDown(document.body, { key: "j" });
    expect(focusedText()).toBe("First");
    await fireEvent.keyDown(document.body, { key: "j" });
    expect(focusedText()).toBe("Second");
    await fireEvent.keyDown(document.body, { key: "k" });
    expect(focusedText()).toBe("First");

    await fireEvent.keyDown(document.activeElement!, { key: "r" });
    expect(notebook.composers.get(discussion.id).replyTo).toBe(posts[0]!.id);
    await waitFor(() => expect(document.activeElement).toBe(composer()));

    // Letters typed into the composer are text, not shortcuts.
    await fireEvent.keyDown(composer(), { key: "j" });
    expect(document.activeElement).toBe(composer());
  });

  it("Ctrl+Enter posts from the composer", async () => {
    fresh();
    seed("", "Question");
    await launch();
    await openDiscussion("Question");
    await fireEvent.input(composer(), { target: { value: "Quick reply" } });
    await fireEvent.keyDown(composer(), { key: "Enter", ctrlKey: true });
    await screen.findByText("Quick reply", { selector: "article p" });
  });
});

describe("settings", () => {
  it("renames the local user everywhere their name appears", async () => {
    fresh();
    seed("", "Question");
    await launch();
    await openDiscussion("Question");

    await fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    const name = screen.getByRole("textbox", { name: "Your name" });
    await fireEvent.input(name, { target: { value: "Thanos" } });
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await screen.findByText("Saved.");
    expect(
      screen.getByRole("button", { name: "Reply to Thanos" }),
    ).toBeTruthy();
    await fireEvent.click(screen.getByRole("tab", { name: "About" }));
    expect(await screen.findByText("/data/nimata.sqlite3")).toBeTruthy();
  });
});
