// Helpers for full-app tests: launch the real App against an in-memory
// backend installed through Tauri's IPC mock, and drive it like a user.

import { expect, vi } from "vitest";
import { screen, within } from "@testing-library/svelte";
import { FakeBackend } from "./fakeBackend";

type TestingLibrary = typeof import("@testing-library/svelte");

export let backend: FakeBackend;
/**
 * Testing Library loaded together with the current copy of the app. Each
 * launch resets the module registry, so rendering, events, and flushing must
 * come from the same Svelte runtime as the app.
 */
let tl: TestingLibrary;

/** Clears modules and loads a fresh app and Testing Library together. */
async function freshModules() {
  tl?.cleanup();
  vi.resetModules();
  tl = await import("@testing-library/svelte");
}

export const fireEvent = new Proxy({} as TestingLibrary["fireEvent"], {
  get: (_, key: keyof TestingLibrary["fireEvent"]) => tl.fireEvent[key],
});
export const waitFor: TestingLibrary["waitFor"] = (...args) =>
  tl.waitFor(...args);

export function fresh() {
  backend = new FakeBackend().install();
}

/** Renders a fresh copy of the app without waiting for it to load. */
export async function mount() {
  await freshModules();
  const { default: App } = await import("../App.svelte");
  const { notebook } = await import("../lib/stores/notebook.svelte");
  tl.render(App);
  return notebook;
}

export function cleanup() {
  tl?.cleanup();
}

/** Mounts a fresh copy of the app, as if Nimata had just been launched. */
export async function launch() {
  const notebook = await mount();
  await waitFor(() => expect(notebook.me).not.toBeNull());
  return notebook;
}

/** Closes the window the way the OS does, then launches the app again. */
export async function restart() {
  window.dispatchEvent(new Event("pagehide"));
  // The draft must be saved on close, not when the typing pause timer fires.
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(backend.calls.some((c) => c.cmd === "save_draft")).toBe(true);
  return launch();
}

export function seed(title: string, body: string) {
  return backend.handle("start_discussion", { title, body }) as {
    discussion: { id: string };
    posts: { id: string }[];
  };
}

export function sidebar() {
  return within(screen.getByRole("navigation", { name: "Discussions" }));
}

export async function openDiscussion(title: string) {
  await fireEvent.click(
    await sidebar().findByRole("button", { name: new RegExp(title) }),
  );
  await screen.findByRole("list", { name: "Posts in order written" });
}

export function composer() {
  return screen.getByRole("textbox", {
    name: "Post text",
  }) as HTMLTextAreaElement;
}

export async function writeAndPost(text: string) {
  await fireEvent.input(composer(), { target: { value: text } });
  await fireEvent.click(screen.getByRole("button", { name: "Post" }));
  await screen.findByText(text, { selector: "article p" });
}

/** The body text of each post, top to bottom. */
export function streamTexts() {
  const list = screen.getByRole("list", { name: "Posts in order written" });
  return [...list.querySelectorAll("article")].map(
    (article) => article.querySelector("div.font-serif p")?.textContent,
  );
}
