<script lang="ts">
  import { tick } from "svelte";
  import { notebook } from "../stores/notebook.svelte";

  let { title, class: className = "" }: { title: string; class?: string } =
    $props();

  let editing = $state(false);
  let value = $state("");
  let input: HTMLInputElement | undefined = $state();

  async function edit(): Promise<void> {
    value = title;
    editing = true;
    await tick();
    input?.select();
  }

  async function save(): Promise<void> {
    if (!editing) return;
    editing = false;
    if (value.trim() !== title) await notebook.rename(value);
  }

  function onKeydown(event: KeyboardEvent): void {
    if (event.key === "Enter") {
      event.preventDefault();
      void save();
    } else if (event.key === "Escape") {
      event.preventDefault();
      editing = false;
    }
  }
</script>

{#if editing}
  <input
    bind:this={input}
    bind:value
    aria-label="Discussion title"
    maxlength="200"
    class="block w-full rounded border border-accent bg-paper px-2 py-0.5 font-serif text-xl focus:outline-none {className}"
    onkeydown={onKeydown}
    onblur={save}
  />
{:else}
  <h1 class="truncate font-serif text-xl {className}">
    <button
      type="button"
      class="max-w-full truncate rounded text-left hover:text-accent"
      title="Rename"
      onclick={edit}
    >
      {title}
    </button>
  </h1>
{/if}
