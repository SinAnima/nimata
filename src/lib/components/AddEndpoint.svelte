<script lang="ts">
  import * as api from "../api";
  import { notebook } from "../stores/notebook.svelte";

  let open = $state(false);
  let name = $state("");
  let url = $state("");
  let key = $state("");
  let error: string | null = $state(null);

  const examples = [
    { name: "Ollama", url: "http://localhost:11434/v1" },
    { name: "LM Studio", url: "http://localhost:1234/v1" },
  ];

  async function add(): Promise<void> {
    error = null;
    try {
      await api.addEndpoint(name, url, key.trim() === "" ? null : key);
      name = "";
      url = "";
      key = "";
      open = false;
      await notebook.loadProviders();
    } catch (e) {
      error = api.errorMessage(e);
    }
  }
</script>

{#if !open}
  <button
    type="button"
    class="min-h-10 rounded border border-rule px-3 text-sm hover:bg-accent-soft"
    onclick={() => (open = true)}
  >
    Add an OpenAI-compatible connection…
  </button>
{:else}
  <form
    class="space-y-3 rounded border border-rule p-4"
    aria-label="Add an OpenAI-compatible connection"
    onsubmit={(e) => {
      e.preventDefault();
      void add();
    }}
  >
    <p class="text-sm text-muted">
      For local models (Ollama, LM Studio, llama.cpp, vLLM) or hosted services
      that speak OpenAI's Chat Completions API.
    </p>
    <div class="flex flex-wrap gap-2 text-sm">
      {#each examples as example (example.name)}
        <button
          type="button"
          class="min-h-8 rounded border border-rule px-2 hover:bg-accent-soft"
          onclick={() => {
            name = example.name;
            url = example.url;
          }}
        >
          {example.name}
        </button>
      {/each}
    </div>
    <label class="block text-sm">
      <span class="text-muted">Name</span>
      <input
        class="mt-1 w-full rounded border border-rule bg-paper px-2 py-1.5 focus:border-accent focus:outline-none"
        bind:value={name}
        placeholder="e.g. Ollama"
      />
    </label>
    <label class="block text-sm">
      <span class="text-muted">Endpoint address</span>
      <input
        class="mt-1 w-full rounded border border-rule bg-paper px-2 py-1.5 font-mono text-xs focus:border-accent focus:outline-none"
        bind:value={url}
        spellcheck="false"
        placeholder="http://localhost:11434/v1"
      />
    </label>
    <label class="block text-sm">
      <span class="text-muted">API key (optional)</span>
      <input
        type="password"
        autocomplete="off"
        class="mt-1 w-full rounded border border-rule bg-paper px-2 py-1.5 font-mono text-xs focus:border-accent focus:outline-none"
        bind:value={key}
      />
    </label>
    {#if error}
      <p class="text-sm" role="alert">{error}</p>
    {/if}
    <div class="flex gap-2">
      <button
        class="min-h-10 rounded bg-accent px-3 text-sm font-medium text-surface disabled:opacity-45"
        disabled={name.trim() === "" || url.trim() === ""}
      >
        Add connection
      </button>
      <button
        type="button"
        class="min-h-10 rounded px-3 text-sm text-muted hover:text-ink"
        onclick={() => (open = false)}
      >
        Cancel
      </button>
    </div>
  </form>
{/if}
