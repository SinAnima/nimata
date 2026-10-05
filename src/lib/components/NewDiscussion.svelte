<script lang="ts">
  import { onMount } from "svelte";
  import { isSubmitShortcut, MODIFIER_LABEL } from "../keys";
  import { notebook } from "../stores/notebook.svelte";

  let { onCancel }: { onCancel: () => void } = $props();

  let title = $state("");
  let body = $state("");
  let starting = $state(false);
  let bodyField: HTMLTextAreaElement | undefined = $state();

  onMount(() => bodyField?.focus());

  async function submit(): Promise<void> {
    if (body.trim() === "" || starting) return;
    starting = true;
    const started = await notebook.start(title, body);
    starting = false;
    if (started) {
      title = "";
      body = "";
    }
  }

  function onKeydown(event: KeyboardEvent): void {
    if (isSubmitShortcut(event)) {
      event.preventDefault();
      void submit();
    } else if (event.key === "Escape") {
      event.preventDefault();
      onCancel();
    }
  }
</script>

<section
  class="flex h-full min-h-0 flex-col bg-surface"
  aria-labelledby="new-title"
>
  <header
    class="flex items-center gap-2 border-b border-rule px-3 pt-[max(0.75rem,env(safe-area-inset-top))] pb-3 sm:px-8"
  >
    <button
      type="button"
      class="min-h-10 rounded px-2 text-accent md:hidden"
      onclick={onCancel}
    >
      <span aria-hidden="true">‹</span> Discussions
    </button>
    <h1 id="new-title" class="font-serif text-xl max-md:hidden">
      New discussion
    </h1>
  </header>

  <form
    class="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-5 py-5 sm:px-8"
    onsubmit={(e) => {
      e.preventDefault();
      void submit();
    }}
  >
    <label class="block">
      <span class="text-sm text-muted">Title</span>
      <input
        class="mt-1 block w-full max-w-3xl rounded border border-rule bg-paper px-3 py-2 font-serif text-xl placeholder:text-muted focus:border-accent focus:outline-none"
        placeholder="Optional. The first line is used if left empty."
        maxlength="200"
        bind:value={title}
        onkeydown={onKeydown}
      />
    </label>
    <label class="flex min-h-0 flex-1 flex-col">
      <span class="text-sm text-muted">First post</span>
      <textarea
        bind:this={bodyField}
        class="mt-1 block min-h-40 w-full max-w-3xl flex-1 resize-none rounded border border-rule bg-paper px-3 py-2 font-serif text-[1.0625rem] leading-[1.6] placeholder:text-muted focus:border-accent focus:outline-none"
        placeholder="What do you want to think through?"
        bind:value={body}
        onkeydown={onKeydown}></textarea>
    </label>
    <div
      class="flex max-w-3xl items-center justify-end gap-3 pb-[env(safe-area-inset-bottom)]"
    >
      <button
        type="button"
        class="min-h-10 rounded px-3 text-sm text-muted hover:text-ink max-md:hidden"
        onclick={onCancel}
      >
        Cancel
      </button>
      <button
        type="submit"
        class="min-h-10 rounded bg-accent px-4 text-sm font-medium text-surface disabled:opacity-45"
        disabled={body.trim() === "" || starting}
        title="Start discussion ({MODIFIER_LABEL}Enter)"
      >
        Start discussion
      </button>
    </div>
  </form>
</section>
