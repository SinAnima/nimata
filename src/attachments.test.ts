// Regression tests for Stage 8: attaching files to posts, previewing them,
// and seeing which models can read them.

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
  sidebar,
  waitFor,
} from "./test/app";

afterEach(cleanup);

const notes = () =>
  new File(["# Notes\n\nStratification matters."], "notes.md");
const pdf = () =>
  new File(["%PDF-1.7 ..."], "paper.pdf", { type: "application/pdf" });

function attachInput(): HTMLInputElement {
  return document.querySelector<HTMLInputElement>("[data-attach-input]")!;
}

async function choose(...files: File[]) {
  await fireEvent.change(attachInput(), { target: { files } });
}

function staged() {
  return within(screen.getByRole("group", { name: "Attached files" }));
}

describe("attachments (Stage 8 demo)", () => {
  it("attaches a Markdown file to a reply, then previews it from the post", async () => {
    fresh();
    seed("Datalog", "Could Datalog replace the rules?");
    await launch();
    await openDiscussion("Datalog");

    await choose(notes());
    await staged().findByRole("button", { name: "Preview notes.md" });
    expect(staged().getByText(/Markdown, 3\d bytes/)).toBeTruthy();
    // A file alone can be posted.
    expect(
      (screen.getByRole("button", { name: "Post" }) as HTMLButtonElement)
        .disabled,
    ).toBe(false);
    await fireEvent.input(composer(), { target: { value: "My notes." } });
    await fireEvent.click(screen.getByRole("button", { name: "Post" }));

    const list = await screen.findByRole("list", {
      name: "Attached files",
    });
    expect(within(list).getByText("notes.md")).toBeTruthy();
    expect(backend.attachments).toHaveLength(1);
    expect(backend.posts.at(-1)!.id).toBe(backend.attachments[0]!.postId);
    expect(screen.queryByRole("group", { name: "Attached files" })).toBeNull();

    await fireEvent.click(
      within(list).getByRole("button", { name: "Preview notes.md" }),
    );
    const dialog = await screen.findByRole("dialog", { name: "notes.md" });
    await within(dialog).findByText(/Stratification matters\./);
    expect(within(dialog).getByRole("button", { name: "Save…" })).toBeTruthy();
  });

  it("accepts files dropped on the composer, and forgets removed ones", async () => {
    fresh();
    seed("Datalog", "Question");
    await launch();
    await openDiscussion("Datalog");
    const form = screen.getByRole("form", { name: "Compose a post" });
    const dataTransfer = { files: [notes(), pdf()], types: ["Files"] };
    await fireEvent.dragOver(form, { dataTransfer });
    await fireEvent.drop(form, { dataTransfer });
    await staged().findByText("paper.pdf");
    expect(backend.blobs.size).toBe(2);

    await fireEvent.click(
      staged().getByRole("button", { name: "Remove paper.pdf" }),
    );
    await waitFor(() => expect(backend.blobs.size).toBe(1));
    expect(staged().queryByText("paper.pdf")).toBeNull();
    // The draft keeps the rest.
    await waitFor(() =>
      expect(
        backend.drafts
          .get(backend.discussions[0]!.id)
          ?.attachments?.map((a) => a.filename),
      ).toEqual(["notes.md"]),
    );
  });

  it("warns when the model about to answer cannot read a file", async () => {
    fresh();
    backend.handle("add_endpoint", {
      displayName: "Ollama",
      baseUrl: "http://localhost:11434/v1",
      key: null,
    });
    backend.handle("set_model", {
      providerId: backend.connections.at(-1)!.id,
      model: "qwen3:8b",
      displayName: "Qwen",
      enabled: true,
    });
    seed("Datalog", "Question");
    await launch();
    await openDiscussion("Datalog");
    await fireEvent.input(composer(), { target: { value: "@qwen thoughts?" } });
    await choose(notes(), pdf());
    await staged().findByText("paper.pdf");
    expect(
      staged().getByText(
        "Qwen cannot read paper.pdf; it will be told the file was left out.",
      ),
    ).toBeTruthy();
  });

  it("starts a discussion with only a file, named after it", async () => {
    fresh();
    await launch();
    await fireEvent.click(
      sidebar().getByRole("button", { name: "Start a discussion" }),
    );
    await choose(notes());
    await staged().findByText("notes.md");
    await fireEvent.click(
      screen.getByRole("button", { name: "Start discussion" }),
    );
    await waitFor(() => expect(backend.discussions[0]?.title).toBe("notes.md"));
    expect(backend.attachments[0]?.filename).toBe("notes.md");
  });

  it("shows images, saves files, and explains a file that cannot be opened", async () => {
    fresh();
    seed("Datalog", "Question");
    backend.dialogPath = "/saved/paper.pdf";
    await launch();
    await openDiscussion("Datalog");
    const png = new File(["\x89PNG..."], "diagram.png", { type: "image/png" });
    await choose(png, pdf(), notes());
    await staged().findByText("notes.md");
    await fireEvent.click(screen.getByRole("button", { name: "Post" }));

    // The image as a thumbnail, opened large.
    const images = await screen.findByRole("list", { name: "Attached images" });
    const thumbnail = within(images).getByRole("img", { name: "diagram.png" });
    expect(thumbnail.getAttribute("src")).toContain("attachment");
    await fireEvent.click(
      within(images).getByRole("button", { name: "Show diagram.png" }),
    );
    let dialog = await screen.findByRole("dialog", { name: "diagram.png" });
    expect(
      within(dialog).getByRole("img", { name: "diagram.png" }),
    ).toBeTruthy();
    await fireEvent.click(
      within(dialog).getByRole("button", { name: "Close" }),
    );

    // A PDF cannot be shown, only saved.
    const files = screen.getByRole("list", { name: "Attached files" });
    expect(
      within(files).queryByRole("button", { name: "Preview paper.pdf" }),
    ).toBeNull();
    await fireEvent.click(
      within(files).getByRole("button", { name: "Save paper.pdf" }),
    );
    await screen.findAllByText("Saved to /saved/paper.pdf");

    // A text file whose bytes are gone says so.
    backend.blobs.clear();
    await fireEvent.click(
      within(files).getByRole("button", { name: "Preview notes.md" }),
    );
    dialog = await screen.findByRole("dialog", { name: "notes.md" });
    expect((await within(dialog).findByRole("alert")).textContent).toContain(
      "not found",
    );
  });

  it("refuses files over the size limit before sending them", async () => {
    fresh();
    seed("Datalog", "Question");
    await launch();
    await openDiscussion("Datalog");
    const huge = notes();
    Object.defineProperty(huge, "size", { value: 60 * 1024 * 1024 });
    await choose(huge);
    await screen.findByText(/notes\.md is larger than 50 MB/);
    expect(backend.blobs.size).toBe(0);
  });

  it("takes dropped files on the new discussion form and forgets removed ones", async () => {
    fresh();
    await launch();
    await fireEvent.click(
      sidebar().getByRole("button", { name: "Start a discussion" }),
    );
    const form = document.querySelector("form")!;
    const dataTransfer = { files: [pdf()], types: ["Files"] };
    await fireEvent.dragOver(form, { dataTransfer });
    await fireEvent.drop(form, { dataTransfer });
    await staged().findByText("paper.pdf");
    await fireEvent.click(
      staged().getByRole("button", { name: "Remove paper.pdf" }),
    );
    await waitFor(() => expect(backend.blobs.size).toBe(0));
  });
});
