<script lang="ts">
  import * as api from "../api";
  import type { ModelInfo, ProviderView } from "../types";
  import { notebook } from "../stores/notebook.svelte";
  import { automaticAlias } from "../mentions";

  let { view }: { view: ProviderView } = $props();

  const provider = $derived(view.provider);
  const key = $derived(view.key);

  let keyInput = $state("");
  let replacing = $state(false);
  let keyError: string | null = $state(null);

  let testing = $state(false);
  let available: ModelInfo[] | null = $state(null);
  let testMessage: string | null = $state(null);
  let testFailed = $state(false);

  let newModel = $state("");
  let modelError: string | null = $state(null);

  const showKeyField = $derived(key.source !== "saved" || replacing);
  const unadded = $derived(
    (available ?? ([] as ModelInfo[])).filter(
      (m) => !view.models.some((p) => p.participant.model === m.id),
    ),
  );

  /** "gpt-5.6-mini" -> "GPT-5.6-mini", the way people write model names. */
  function defaultName(id: string): string {
    return id.replace(/^chatgpt/i, "ChatGPT").replace(/^gpt/i, "GPT");
  }

  async function saveKey(): Promise<void> {
    keyError = null;
    try {
      await api.saveApiKey(provider.id, keyInput);
      keyInput = "";
      replacing = false;
      await notebook.loadProviders();
    } catch (e) {
      keyError = api.errorMessage(e);
    }
  }

  async function removeKey(): Promise<void> {
    keyError = null;
    try {
      await api.removeApiKey(provider.id);
      available = null;
      testMessage = null;
      await notebook.loadProviders();
    } catch (e) {
      keyError = api.errorMessage(e);
    }
  }

  async function testConnection(): Promise<void> {
    testing = true;
    testMessage = null;
    try {
      available = await api.providerModels(provider.id);
      testFailed = false;
      testMessage = `Connected. ${available.length} text ${available.length === 1 ? "model" : "models"} available.`;
    } catch (e) {
      available = null;
      testFailed = true;
      testMessage = api.errorMessage(e);
    } finally {
      testing = false;
    }
  }

  async function setAliases(
    participantId: string,
    text: string,
  ): Promise<void> {
    modelError = null;
    try {
      const aliases = text.split(/[\s,]+/).filter((a) => a !== "");
      await api.setModelAliases(participantId, aliases);
      await notebook.loadProviders();
    } catch (e) {
      modelError = api.errorMessage(e);
    }
  }

  async function setModel(
    model: string,
    displayName: string,
    enabled: boolean,
  ): Promise<void> {
    modelError = null;
    try {
      await api.setModel(provider.id, model, displayName, enabled);
      await notebook.loadProviders();
    } catch (e) {
      modelError = api.errorMessage(e);
    }
  }

  async function addModel(): Promise<void> {
    const id = newModel.trim();
    if (id === "") return;
    await setModel(id, defaultName(id), true);
    if (!modelError) newModel = "";
  }
</script>

<section aria-labelledby="provider-{provider.id}" class="space-y-4">
  <div class="flex items-baseline justify-between gap-3">
    <h3 id="provider-{provider.id}" class="font-serif text-lg">
      {provider.displayName}
    </h3>
    <span class="text-sm text-muted">
      {key.source ? "Key ready" : "No key yet"}
    </span>
  </div>

  <div>
    <h4 class="text-sm font-medium">API key</h4>
    {#if key.source === "saved"}
      <p class="mt-1 text-sm text-muted">
        Saved in {key.store}{key.hint ? `, ending in ${key.hint}` : ""}.
      </p>
    {:else if key.source === "environment"}
      <p class="mt-1 text-sm text-muted">
        Using {key.environmentVariable} from your environment{key.hint
          ? `, ending in ${key.hint}`
          : ""}. This happens only in development builds; save a key to use it
        everywhere.
      </p>
    {:else}
      <p class="mt-1 text-sm text-muted">
        Paste a key from your {provider.displayName} account. It is saved in {key.store},
        never in Nimata's database, backups, or exports.
      </p>
    {/if}

    {#if showKeyField}
      <form
        class="mt-2 flex gap-2"
        onsubmit={(e) => {
          e.preventDefault();
          void saveKey();
        }}
      >
        <label class="min-w-0 flex-1">
          <span class="sr-only">{provider.displayName} API key</span>
          <input
            type="password"
            autocomplete="off"
            spellcheck="false"
            placeholder="sk-…"
            class="w-full rounded border border-rule bg-paper px-3 py-2 font-mono text-sm focus:border-accent focus:outline-none"
            bind:value={keyInput}
          />
        </label>
        <button
          class="min-h-10 rounded border border-rule px-3 text-sm font-medium text-accent hover:bg-accent-soft disabled:opacity-45"
          disabled={keyInput.trim() === ""}
        >
          Save key
        </button>
        {#if replacing}
          <button
            type="button"
            class="min-h-10 rounded px-2 text-sm text-muted hover:text-ink"
            onclick={() => {
              replacing = false;
              keyInput = "";
            }}
          >
            Cancel
          </button>
        {/if}
      </form>
    {/if}

    {#if key.source === "saved" && !replacing}
      <div class="mt-2 flex flex-wrap gap-2">
        <button
          type="button"
          class="min-h-10 rounded border border-rule px-3 text-sm hover:bg-accent-soft"
          onclick={() => (replacing = true)}
        >
          Replace key
        </button>
        <button
          type="button"
          class="min-h-10 rounded border border-rule px-3 text-sm hover:bg-accent-soft"
          onclick={removeKey}
        >
          Remove key
        </button>
      </div>
    {/if}
    {#if keyError}
      <p class="mt-1 text-sm" role="alert">{keyError}</p>
    {/if}
  </div>

  {#if key.source}
    <div>
      <button
        type="button"
        class="min-h-10 rounded border border-rule px-3 text-sm hover:bg-accent-soft disabled:opacity-45"
        disabled={testing}
        onclick={testConnection}
      >
        {testing ? "Testing…" : "Test connection"}
      </button>
      {#if testMessage}
        <p
          class="mt-1 text-sm text-muted"
          role={testFailed ? "alert" : "status"}
        >
          {testMessage}
        </p>
      {/if}
    </div>
  {/if}

  <div>
    <h4 class="text-sm font-medium">Models in your discussions</h4>
    {#if view.models.length === 0}
      <p class="mt-1 text-sm text-muted">
        Add a model to ask it for replies. Each model becomes a participant with
        its own name.
      </p>
    {/if}
    <ul class="mt-2 space-y-2">
      {#each view.models as model (model.participant.id)}
        <li class="rounded border border-rule px-3 py-2">
          <div class="flex items-center gap-3">
            <label class="flex min-h-10 items-center">
              <input
                type="checkbox"
                class="size-4 accent-[var(--accent)]"
                checked={model.enabled}
                aria-label="Use {model.participant.displayName} in discussions"
                onchange={(e) =>
                  setModel(
                    model.participant.model ?? "",
                    model.participant.displayName,
                    e.currentTarget.checked,
                  )}
              />
            </label>
            <label class="min-w-0 flex-1">
              <span class="sr-only"
                >Name shown for {model.participant.model}</span
              >
              <input
                class="w-full rounded border border-rule bg-paper px-2 py-1.5 text-sm focus:border-accent focus:outline-none"
                value={model.participant.displayName}
                maxlength="80"
                onchange={(e) =>
                  setModel(
                    model.participant.model ?? "",
                    e.currentTarget.value,
                    model.enabled,
                  )}
              />
            </label>
            <span
              class="w-36 shrink-0 truncate text-xs text-muted"
              title={model.participant.model}
            >
              {model.participant.model}
            </span>
          </div>
          <label class="mt-1 flex items-center gap-2 pl-7 text-sm">
            <span class="shrink-0 text-muted">Aliases</span>
            <input
              class="min-w-0 flex-1 rounded border border-rule bg-paper px-2 py-1 focus:border-accent focus:outline-none"
              value={model.aliases.join(", ")}
              placeholder="e.g. review, r"
              spellcheck="false"
              aria-label="Aliases for {model.participant.displayName}"
              onchange={(e) =>
                setAliases(model.participant.id, e.currentTarget.value)}
            />
            <span class="shrink-0 text-xs text-muted">
              and @{automaticAlias(model.participant.displayName)}
            </span>
          </label>
        </li>
      {/each}
    </ul>

    <form
      class="mt-3 flex gap-2"
      onsubmit={(e) => {
        e.preventDefault();
        void addModel();
      }}
    >
      <label class="min-w-0 flex-1">
        <span class="sr-only">Model to add</span>
        {#if unadded.length > 0}
          <select
            class="w-full rounded border border-rule bg-paper px-2 py-2 text-sm"
            bind:value={newModel}
          >
            <option value="">Choose a model…</option>
            {#each unadded as m (m.id)}
              <option value={m.id}>{m.id}</option>
            {/each}
          </select>
        {:else}
          <input
            class="w-full rounded border border-rule bg-paper px-3 py-2 text-sm focus:border-accent focus:outline-none"
            placeholder={available
              ? "Model ID"
              : "Model ID, or test the connection to choose"}
            spellcheck="false"
            bind:value={newModel}
          />
        {/if}
      </label>
      <button
        class="min-h-10 rounded border border-rule px-3 text-sm font-medium text-accent hover:bg-accent-soft disabled:opacity-45"
        disabled={newModel.trim() === ""}
      >
        Add model
      </button>
    </form>
    {#if modelError}
      <p class="mt-1 text-sm" role="alert">{modelError}</p>
    {/if}
  </div>
</section>
