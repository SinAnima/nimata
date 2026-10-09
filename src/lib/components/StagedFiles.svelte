<script lang="ts">
  import type { ModelParticipant, StagedAttachment } from "../types";
  import { attachmentUrl } from "../api";
  import { humanSize, typeLabel, unreadable } from "../attach";
  import { notebook } from "../stores/notebook.svelte";
  import {
    previewable,
    previewAttachment,
  } from "../stores/attachmentView.svelte";

  interface Props {
    files: StagedAttachment[];
    /** Files still being stored. */
    staging: number;
    /** The models about to be asked, to warn about files they cannot read. */
    answerers: readonly ModelParticipant[];
    onRemove: (index: number) => void;
  }

  let { files, staging, answerers, onRemove }: Props = $props();

  const warnings = $derived(
    unreadable(answerers, notebook.providers, files).map(
      ({ model, files }) =>
        `${model.participant.displayName} cannot read ${files.map((f) => f.filename).join(", ")}; it will be told ${files.length === 1 ? "the file was" : "the files were"} left out.`,
    ),
  );
</script>

{#if files.length > 0 || staging > 0}
  <div class="mt-1.5" role="group" aria-label="Attached files">
    <ul class="flex flex-wrap items-center gap-1.5 text-sm">
      {#each files as file, i (i)}
        <li
          class="inline-flex max-w-full items-center gap-1.5 rounded border border-rule bg-paper py-0.5 pr-1 pl-1.5"
        >
          {#if file.kind === "image"}
            <img
              src={attachmentUrl(file.contentHash)}
              alt=""
              class="h-6 w-6 rounded-sm object-cover"
            />
          {/if}
          {#if previewable(file)}
            <button
              type="button"
              class="min-w-0 truncate rounded hover:underline"
              aria-label="Preview {file.filename}"
              onclick={() => previewAttachment(file)}
            >
              {file.filename}
            </button>
          {:else}
            <span class="min-w-0 truncate">{file.filename}</span>
          {/if}
          <span class="shrink-0 text-muted"
            >{typeLabel(file)}, {humanSize(file.size)}</span
          >
          <button
            type="button"
            class="min-h-7 shrink-0 rounded px-1 text-muted hover:text-ink"
            aria-label="Remove {file.filename}"
            onclick={() => onRemove(i)}
          >
            ×
          </button>
        </li>
      {/each}
      {#if staging > 0}
        <li class="text-muted" role="status">
          Adding {staging === 1 ? "a file" : `${staging} files`}…
        </li>
      {/if}
    </ul>
    {#each warnings as warning (warning)}
      <p class="mt-1 text-sm text-muted">{warning}</p>
    {/each}
  </div>
{/if}
