<script lang="ts">
  import type { Attachment } from "../types";
  import { attachmentUrl } from "../api";
  import { humanSize, typeLabel } from "../attach";
  import { notebook } from "../stores/notebook.svelte";
  import {
    previewable,
    previewAttachment,
  } from "../stores/attachmentView.svelte";

  let { attachments }: { attachments: Attachment[] } = $props();

  const images = $derived(attachments.filter((a) => a.kind === "image"));
  const files = $derived(attachments.filter((a) => a.kind !== "image"));
</script>

<div class="mt-3 max-w-[68ch]">
  {#if images.length > 0}
    <ul class="flex flex-wrap gap-2" aria-label="Attached images">
      {#each images as image (image.id)}
        <li>
          <button
            type="button"
            class="block overflow-hidden rounded border border-rule hover:border-accent"
            aria-label="Show {image.filename}"
            onclick={() => previewAttachment(image)}
          >
            <img
              src={attachmentUrl(image.contentHash)}
              alt={image.filename}
              loading="lazy"
              class="block max-h-48 max-w-[min(20rem,70vw)] object-contain"
            />
          </button>
        </li>
      {/each}
    </ul>
  {/if}
  {#if files.length > 0}
    <ul class="mt-2 space-y-1" aria-label="Attached files">
      {#each files as file (file.id)}
        <li
          class="flex flex-wrap items-center gap-x-3 rounded border border-rule px-3 py-1.5 text-sm"
        >
          <span class="min-w-0 flex-1 truncate font-medium"
            >{file.filename}</span
          >
          <span class="text-muted"
            >{typeLabel(file)}, {humanSize(file.size)}</span
          >
          {#if previewable(file)}
            <button
              type="button"
              class="min-h-8 rounded px-1 text-accent hover:underline"
              aria-label="Preview {file.filename}"
              onclick={() => previewAttachment(file)}
            >
              Preview
            </button>
          {/if}
          <button
            type="button"
            class="min-h-8 rounded px-1 text-accent hover:underline"
            aria-label="Save {file.filename}"
            onclick={() => notebook.saveAttachment(file.id)}
          >
            Save…
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>
