<script lang="ts">
  import { tick } from "svelte";
  import { appInfo, setDefaultModel as setDefault } from "../api";
  import type { AppInfo } from "../types";
  import { notebook } from "../stores/notebook.svelte";
  import ProviderSettings from "./ProviderSettings.svelte";
  import AddEndpoint from "./AddEndpoint.svelte";

  type Tab = "general" | "models" | "data" | "about";
  const tabs: { id: Tab; label: string }[] = [
    { id: "general", label: "General" },
    { id: "models", label: "Models" },
    { id: "data", label: "Data" },
    { id: "about", label: "About" },
  ];

  let dialog: HTMLDialogElement | undefined = $state();
  let tab: Tab = $state("general");
  let info: AppInfo | null = $state(null);
  let name = $state("");
  let saved = $state(false);

  export async function open(start: Tab = "general"): Promise<void> {
    name = notebook.me?.displayName ?? "";
    saved = false;
    tab = start;
    dialog?.showModal();
    void notebook.loadProviders();
    info ??= await appInfo().catch(() => null);
  }

  async function restore(): Promise<void> {
    if (await notebook.restore()) dialog?.close();
  }

  async function setDefaultModel(id: string | null): Promise<void> {
    try {
      await setDefault(id);
      notebook.defaultModelId = id;
    } catch (e) {
      notebook.report(e);
    }
  }

  async function saveName(): Promise<void> {
    saved = await notebook.renameMe(name);
  }

  /** Arrow keys move between tabs, as in native tab bars. */
  async function onTabKeydown(event: KeyboardEvent): Promise<void> {
    const step = { ArrowRight: 1, ArrowLeft: -1 }[event.key];
    if (!step) return;
    event.preventDefault();
    const index = tabs.findIndex((t) => t.id === tab);
    tab = tabs[(index + step + tabs.length) % tabs.length]!.id;
    await tick();
    document.getElementById(`settings-tab-${tab}`)?.focus();
  }
</script>

<dialog
  bind:this={dialog}
  aria-labelledby="settings-title"
  class="m-auto w-[min(40rem,calc(100vw-1rem))] rounded-lg border border-rule bg-surface p-0 text-ink shadow-xl backdrop:bg-black/30"
>
  <div class="px-6 pt-5">
    <h2 id="settings-title" class="font-serif text-2xl">Settings</h2>
    <div
      role="tablist"
      aria-label="Settings sections"
      tabindex="-1"
      class="-mx-2 mt-3 flex gap-1 overflow-x-auto border-b border-rule"
      onkeydown={onTabKeydown}
    >
      {#each tabs as t (t.id)}
        <button
          type="button"
          role="tab"
          id="settings-tab-{t.id}"
          aria-selected={tab === t.id}
          aria-controls="settings-panel"
          tabindex={tab === t.id ? 0 : -1}
          class={[
            "-mb-px min-h-10 border-b-2 px-3 text-sm",
            tab === t.id
              ? "border-accent font-medium text-ink"
              : "border-transparent text-muted hover:text-ink",
          ]}
          onclick={() => (tab = t.id)}
        >
          {t.label}
        </button>
      {/each}
    </div>
  </div>

  <div
    id="settings-panel"
    role="tabpanel"
    aria-labelledby="settings-tab-{tab}"
    class="max-h-[min(70vh,36rem)] overflow-y-auto px-6 py-5"
  >
    {#if tab === "general"}
      <form
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
    {:else if tab === "models"}
      <p class="mb-4 text-sm leading-relaxed text-muted">
        Models take part in your discussions as participants. Only the thread
        you ask a model to reply to is sent to its provider; your discussions
        stay on this device.
      </p>
      {#if notebook.models.length > 0}
        <label class="mb-5 flex flex-wrap items-center gap-2 text-sm">
          <span>When no model is mentioned or replying, ask</span>
          <select
            class="rounded border border-rule bg-paper px-2 py-1"
            value={notebook.defaultModelId ?? ""}
            onchange={(e) => setDefaultModel(e.currentTarget.value || null)}
          >
            <option value="">No one</option>
            {#each notebook.models.filter((m) => m.enabled) as model (model.participant.id)}
              <option value={model.participant.id}>
                {model.participant.displayName}
              </option>
            {/each}
          </select>
        </label>
      {/if}
      {#each notebook.providers as view, i (view.provider.id)}
        {#if i > 0}<hr class="my-6 border-rule" />{/if}
        <ProviderSettings {view} />
      {:else}
        <p class="text-sm text-muted">Loading…</p>
      {/each}
      <div class="mt-6">
        <AddEndpoint />
      </div>
    {:else if tab === "data"}
      <p class="text-sm leading-relaxed text-muted">
        A backup is a single file containing every discussion, post, and earlier
        version. Keep it somewhere other than this device. API keys are not
        included; add them again after restoring on another device.
      </p>
      <div class="mt-3 flex flex-wrap gap-2">
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
    {:else}
      <p class="font-serif leading-relaxed">
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
    {/if}
  </div>

  <form method="dialog" class="flex justify-end border-t border-rule px-6 py-3">
    <button
      class="min-h-10 rounded px-4 text-sm font-medium text-accent hover:bg-accent-soft"
    >
      Close
    </button>
  </form>
</dialog>
