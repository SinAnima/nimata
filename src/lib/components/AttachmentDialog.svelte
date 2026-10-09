<script lang="ts">
  import { attachmentUrl } from "../api";
  import { humanSize, typeLabel } from "../attach";
  import { notebook } from "../stores/notebook.svelte";
  import {
    attachmentView,
    closePreview,
  } from "../stores/attachmentView.svelte";

  let dialog: HTMLDialogElement | undefined = $state();

  $effect(() => {
    if (attachmentView.file && !dialog?.open) dialog?.showModal();
    if (!attachmentView.file && dialog?.open) dialog.close();
  });
</script>

<dialog
  bind:this={dialog}
  aria-labelledby="attachment-title"
  class="m-auto w-[min(56rem,calc(100vw-1rem))] rounded-lg border border-rule bg-surface p-0 text-ink shadow-xl backdrop:bg-black/30"
  onclose={closePreview}
>
  {#if attachmentView.file}
    {@const file = attachmentView.file}
    <div class="max-h-[min(85vh,56rem)] overflow-y-auto px-6 pt-5 pb-4">
      <h2 id="attachment-title" class="font-serif text-xl break-all">
        {file.filename}
      </h2>
      <p class="mt-1 text-sm text-muted">
        {typeLabel(file)}, {humanSize(file.size)}
      </p>

      {#if file.kind === "image"}
        <img
          src={attachmentUrl(file.contentHash)}
          alt={file.filename}
          class="mt-4 max-h-[65vh] max-w-full rounded border border-rule object-contain"
        />
      {:else if attachmentView.error}
        <p class="mt-4 text-sm" role="alert">{attachmentView.error}</p>
      {:else if attachmentView.text}
        <pre
          class="mt-4 overflow-x-auto rounded border border-rule bg-paper p-3 font-mono text-sm leading-relaxed whitespace-pre-wrap">{attachmentView
            .text.text}</pre>
        {#if attachmentView.text.truncated}
          <p class="mt-2 text-sm text-muted">
            Only the beginning is shown. Save the file to read all of it.
          </p>
        {/if}
      {:else if file.kind === "text"}
        <p class="mt-4 text-sm text-muted">Opening…</p>
      {:else}
        <p class="mt-4 text-sm text-muted">
          Nimata cannot show this kind of file. Save it to open it with another
          app.
        </p>
      {/if}
    </div>
    <div class="flex justify-end gap-2 border-t border-rule px-6 py-3">
      {#if file.id}
        <button
          type="button"
          class="min-h-10 rounded px-4 text-sm hover:bg-accent-soft"
          onclick={() => file.id && notebook.saveAttachment(file.id)}
        >
          Save…
        </button>
      {/if}
      <button
        type="button"
        class="min-h-10 rounded px-4 text-sm font-medium text-accent hover:bg-accent-soft"
        onclick={closePreview}
      >
        Close
      </button>
    </div>
  {/if}
</dialog>
