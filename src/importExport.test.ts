// Regression tests for Stage 7: exporting a discussion as Markdown, the
// full archive, and importing from the Data settings.

import { afterEach, describe, expect, it } from "vitest";
import { screen, within } from "@testing-library/svelte";
import {
  backend,
  cleanup,
  fireEvent,
  fresh,
  launch,
  openDiscussion,
  seed,
  sidebar,
  waitFor,
} from "./test/app";

afterEach(cleanup);

async function openDataTab() {
  await fireEvent.click(screen.getByRole("button", { name: "Settings" }));
  await fireEvent.click(screen.getByRole("tab", { name: "Data" }));
}

describe("export and import (Stage 7 demo)", () => {
  it("exports the open discussion as Markdown from its menu", async () => {
    fresh();
    seed("CouchDB?", "Should Nimata use CouchDB?");
    backend.dialogPath = "/exports/CouchDB.md";
    await launch();
    await openDiscussion("CouchDB");
    await fireEvent.click(screen.getByText("More"));
    await fireEvent.click(
      screen.getByRole("button", { name: "Export as Markdown…" }),
    );
    await screen.findByText("Exported to /exports/CouchDB.md");
    expect(backend.exports.get("/exports/CouchDB.md")).toBe("# CouchDB?\n");
  });

  it("exports every discussion into one archive", async () => {
    fresh();
    seed("One", "First");
    seed("Two", "Second");
    backend.dialogPath = "/exports/nimata-archive.zip";
    await launch();
    await openDataTab();
    await fireEvent.click(
      screen.getByRole("button", { name: "Export all discussions…" }),
    );
    await screen.findAllByText(
      "Exported 2 discussions to /exports/nimata-archive.zip",
    );
  });

  it("imports a ChatGPT export and lists the new discussions", async () => {
    fresh();
    backend.importable = [
      { title: "Datalog", body: "Could Datalog replace the rules?" },
    ];
    await launch();
    await openDataTab();
    await fireEvent.click(screen.getByRole("button", { name: "Import…" }));
    const dialog = screen.getByRole("dialog", { name: "Settings" });
    await within(dialog).findByText(
      "Imported 1 new discussion (1 post) from the ChatGPT export.",
    );
    await fireEvent.click(
      within(dialog).getByRole("button", { name: "Close" }),
    );
    await waitFor(() =>
      expect(sidebar().getByRole("button", { name: /Datalog/ })).toBeTruthy(),
    );
  });

  it("does nothing when the file choice is cancelled", async () => {
    fresh();
    backend.dialogPath = null;
    await launch();
    await openDataTab();
    await fireEvent.click(screen.getByRole("button", { name: "Import…" }));
    await waitFor(() =>
      expect(backend.calls.some((c) => c.cmd === "import_file")).toBe(true),
    );
    expect(screen.queryByText(/Imported/)).toBeNull();
  });
});
