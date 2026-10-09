<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { isSubmitShortcut, MODIFIER_LABEL } from "../keys";
  import { notebook } from "../stores/notebook.svelte";
  import { whoAnswers, type AskChoice } from "../compose";
  import AskLine from "./AskLine.svelte";
  import StagedFiles from "./StagedFiles.svelte";
  import type { StagedAttachment } from "../types";
  import { carriesFiles, droppedFiles } from "../attach";

  let { onCancel }: { onCancel: () => void } = $props();

  let title = $state("");
  let body = $state("");
  let starting = $state(false);
  let choice: AskChoice = $state(null);
  const answerers = $derived(
    whoAnswers({
      text: body,
      choice,
      target: undefined,
      posts: [],
      askable: notebook.askableModels,
      defaultModelId: notebook.defaultModelId,
    }),
  );
  let bodyField: HTMLTextAreaElement | undefined = $state();

  /** Files for the first post. Nothing keeps them until it is posted. */
  let files: StagedAttachment[] = $state([]);
  let staging = $state(0);
  let dragging = $state(false);
  let picker: HTMLInputElement | undefined = $state();

  const canStart = $derived(
    (body.trim() !== "" || files.length > 0) &&
      !starting &&
      staging === 0 &&
      answerers.mentions.problems.length === 0,
  );

  onMount(() => bodyField?.focus());

  // Files added but never posted are deleted when the form goes away.
  onDestroy(() => {
    for (const file of files) void notebook.discardStaged(file);
  });

  async function attach(chosen: File[]): Promise<void> {
    await notebook.stageFiles(
      chosen,
      (file) => (files = [...files, file]),
      (remaining) => (staging = remaining),
    );
  }

  async function submit(): Promise<void> {
    if (!canStart) return;
    starting = true;
    const started = await notebook.start(title, body, answerers.models, files);
    starting = false;
    if (started) {
      title = "";
      body = "";
      choice = null;
      files = [];
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
    class={[
      "flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-5 py-5 sm:px-8",
      dragging &&
        "bg-accent-soft/60 outline-2 outline-accent outline-dashed -outline-offset-4",
    ]}
    ondragover={(e) => {
      if (!carriesFiles(e)) return;
      e.preventDefault();
      dragging = true;
    }}
    ondragleave={() => (dragging = false)}
    ondrop={(e) => {
      if (!carriesFiles(e)) return;
      e.preventDefault();
      dragging = false;
      void attach(droppedFiles(e));
    }}
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
    <div class="max-w-3xl">
      <StagedFiles
        {files}
        {staging}
        answerers={answerers.models}
        onRemove={(i) => {
          const [removed] = files.splice(i, 1);
          files = [...files];
          if (removed) void notebook.discardStaged(removed);
        }}
      />
      <AskLine {answerers} bind:choice />
    </div>
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
      <input
        bind:this={picker}
        type="file"
        multiple
        class="hidden"
        aria-hidden="true"
        tabindex="-1"
        data-attach-input
        onchange={(e) => {
          const chosen = [...(e.currentTarget.files ?? [])];
          e.currentTarget.value = "";
          void attach(chosen);
        }}
      />
      <button
        type="button"
        class="mr-auto min-h-10 rounded border border-rule px-3 text-sm hover:bg-accent-soft"
        title="Attach files (or drop them here)"
        onclick={() => picker?.click()}
      >
        Attach…
      </button>
      <button
        type="submit"
        class="min-h-10 rounded bg-accent px-4 text-sm font-medium text-surface disabled:opacity-45"
        disabled={!canStart}
        title="Start discussion ({MODIFIER_LABEL}Enter)"
      >
        Start discussion
      </button>
    </div>
  </form>
</section>
