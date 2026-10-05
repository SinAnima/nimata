<script lang="ts">
  import { appInfo } from "../api";
  import type { AppInfo } from "../types";
  import { notebook } from "../stores/notebook.svelte";

  let dialog: HTMLDialogElement | undefined = $state();
  let info: AppInfo | null = $state(null);
  let name = $state("");
  let saved = $state(false);

  export async function open(): Promise<void> {
    name = notebook.me?.displayName ?? "";
    saved = false;
    dialog?.showModal();
    info ??= await appInfo().catch(() => null);
  }

  async function restore(): Promise<void> {
    if (await notebook.restore()) dialog?.close();
  }

  async function saveName(): Promise<void> {
    saved = await notebook.renameMe(name);
  }
</script>

<dialog
  bind:this={dialog}
  aria-labelledby="settings-title"
  class="m-auto w-[min(32rem,calc(100vw-2rem))] rounded-lg border border-rule bg-surface p-0 text-ink shadow-xl backdrop:bg-black/30"
>
  <div class="p-6">
    <h2 id="settings-title" class="font-serif text-2xl">Settings</h2>

    <form
      class="mt-4"
      onsubmit={(e) => {
        e.preventDefault();
        void saveName();
      }}
    >
      <label for="display-name" class="text-sm text-muted">Your name</label>
      <div class="mt-1 flex gap-2">
        <input
          id="display-name"
          class="min-w-0 flex-1 rounded border border-rule bg-paper px-3 py-2 focus:border-accent focus:outline-none"
          maxlength="80"
          bind:value={name}
          oninput={() => (saved = false)}
        />
        <button
          class="min-h-10 rounded border border-rule px-3 text-sm font-medium text-accent hover:bg-accent-soft disabled:opacity-45"
          disabled={name.trim() === "" ||
            name.trim() === notebook.me?.displayName}
        >
          Save
        </button>
      </div>
      <p class="mt-1 text-sm text-muted" aria-live="polite">
        {saved ? "Saved." : "Shown as the author of your posts."}
      </p>
    </form>

    <h3 class="mt-6 font-serif text-lg">Your data</h3>
    <p class="mt-1 text-sm leading-relaxed text-muted">
      A backup is a single file containing every discussion, post, and earlier
      version. Keep it somewhere other than this device.
    </p>
    <div class="mt-2 flex flex-wrap gap-2">
      <button
        type="button"
        class="min-h-10 rounded border border-rule px-3 text-sm font-medium text-accent hover:bg-accent-soft"
        onclick={() => notebook.backup()}
      >
        Back up…
      </button>
      <button
        type="button"
        class="min-h-10 rounded border border-rule px-3 text-sm hover:bg-accent-soft"
        onclick={restore}
      >
        Restore from backup…
      </button>
    </div>
    {#if notebook.status}
      <p class="mt-2 text-sm break-all text-muted" role="status">
        {notebook.status}
      </p>
    {/if}

    <h3 class="mt-6 font-serif text-lg">About Nimata</h3>
    <p class="mt-1 font-serif leading-relaxed">
      A local-first discussion client for conversations with people and AI
      models. Your discussions are stored on this device and belong to you.
    </p>

    <dl class="mt-4 grid grid-cols-[auto_1fr] gap-x-6 gap-y-1.5 text-sm">
      <dt class="text-muted">Version</dt>
      <dd class="tabular-nums">{info?.version ?? "Unavailable"}</dd>
      <dt class="text-muted">Tauri</dt>
      <dd class="tabular-nums">{info?.tauriVersion ?? "Unavailable"}</dd>
      <dt class="text-muted">Platform</dt>
      <dd>{info?.platform ?? "Unavailable"}</dd>
      <dt class="text-muted">Database</dt>
      <dd class="break-all">{info?.databasePath ?? "Unavailable"}</dd>
      <dt class="text-muted">Appearance</dt>
      <dd>Follows your system light or dark setting</dd>
      <dt class="text-muted">Privacy</dt>
      <dd>No account, no telemetry</dd>
    </dl>
  </div>
  <form method="dialog" class="flex justify-end border-t border-rule px-6 py-3">
    <button
      class="min-h-10 rounded px-4 text-sm font-medium text-accent hover:bg-accent-soft"
    >
      Close
    </button>
  </form>
</dialog>
