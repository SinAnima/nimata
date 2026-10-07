// Regression tests for Stage 6: searching every discussion from the
// sidebar and jumping to the matching post.

import { afterEach, describe, expect, it } from "vitest";
import { screen, within } from "@testing-library/svelte";
import {
  backend,
  cleanup,
  fireEvent,
  fresh,
  launch,
  seed,
  sidebar,
  waitFor,
} from "./test/app";

afterEach(cleanup);

function searchBox() {
  return screen.getByRole("searchbox", { name: "Search all discussions" });
}

async function searchFor(query: string) {
  searchBox().focus();
  await fireEvent.input(searchBox(), { target: { value: query } });
  await fireEvent.keyDown(searchBox(), { key: "Enter" });
  return within(await screen.findByRole("region", { name: "Search results" }));
}

function corpus() {
  seed(
    "Datalog for the mapping engine",
    "Could Datalog replace the Oracle rules?",
  );
  const { discussion, posts } = seed("Νήματα", "Threads, in Greek: νήματα.");
  backend.handle("add_post", {
    discussionId: discussion.id,
    parentId: posts[0]!.id,
    body: "Stratification handles the overrides.",
    contextIds: [],
  });
}

describe("search (Stage 6 demo)", () => {
  it("finds a post by the beginning of a word and opens it highlighted", async () => {
    fresh();
    corpus();
    await launch();
    const results = await searchFor("stratif");

    const hit = results.getByRole("button", { name: /Stratification handles/ });
    expect(hit.querySelector("mark")?.textContent).toBe("Stratification");
    expect(within(hit).getByText("Νήματα")).toBeTruthy();
    await fireEvent.click(hit);

    const list = await screen.findByRole("list", {
      name: "Posts in order written",
    });
    await waitFor(() =>
      expect(within(list).getByText("Stratification").tagName).toBe("MARK"),
    );
    // Words in other discussions are not highlighted, nor other words.
    expect(list.querySelectorAll("mark")).toHaveLength(1);

    // Closing search brings the list back and removes the highlight.
    await fireEvent.click(
      sidebar().getByRole("button", { name: "Close search" }),
    );
    expect(list.querySelectorAll("mark")).toHaveLength(0);
    expect(
      sidebar().getByRole("button", { name: /Datalog for the mapping/ }),
    ).toBeTruthy();
  });

  it("ignores accents and finds titles", async () => {
    fresh();
    corpus();
    await launch();
    const results = await searchFor("νηματα");
    const titles = within(results.getByRole("region", { name: "Discussions" }));
    expect(titles.getByRole("button", { name: /Νήματα/ })).toBeTruthy();
    expect(
      results.getByRole("button", { name: /Threads, in Greek/ }),
    ).toBeTruthy();
  });

  it("says when nothing matches and offers tips", async () => {
    fresh();
    corpus();
    await launch();
    const results = await searchFor("kubernetes");
    expect(results.getByText("Nothing matches.")).toBeTruthy();
    expect(results.getByText("Search tips")).toBeTruthy();
    expect(sidebar().getByText("No matches")).toBeTruthy();
  });

  it("remembers searches that were used and offers them again", async () => {
    fresh();
    corpus();
    await launch();
    const results = await searchFor("oracle");
    await fireEvent.click(
      results.getByRole("button", { name: /Could Datalog/ }),
    );
    await waitFor(() => expect(backend.recent).toEqual(["oracle"]));

    await fireEvent.keyDown(searchBox(), { key: "Escape" });
    expect((searchBox() as HTMLInputElement).value).toBe("");
    searchBox().focus();
    const recent = within(
      await screen.findByRole("region", { name: "Recent searches" }),
    );
    await fireEvent.click(recent.getByRole("button", { name: "oracle" }));
    expect((searchBox() as HTMLInputElement).value).toBe("oracle");
    await screen.findByRole("region", { name: "Search results" });
  });

  it("focuses search with the keyboard shortcut", async () => {
    fresh();
    corpus();
    await launch();
    await fireEvent.keyDown(window, { key: "f", ctrlKey: true });
    expect(document.activeElement).toBe(searchBox());
    searchBox().blur();
    await fireEvent.keyDown(document.body, { key: "/" });
    expect(document.activeElement).toBe(searchBox());
  });
});
