<script lang="ts">
  import { answer, confirmation } from "../stores/confirmation.svelte";

  let dialog: HTMLDialogElement | undefined = $state();

  $effect(() => {
    if (confirmation.question && !dialog?.open) dialog?.showModal();
    if (!confirmation.question && dialog?.open) dialog.close();
  });
</script>

<dialog
  bind:this={dialog}
  aria-labelledby="confirm-title"
  aria-describedby="confirm-message"
  class="m-auto w-[min(28rem,calc(100vw-2rem))] rounded-lg border border-rule bg-surface p-0 text-ink shadow-xl backdrop:bg-black/30"
  oncancel={(e) => {
    e.preventDefault();
    answer(false);
  }}
>
  {#if confirmation.question}
    <div class="p-6">
      <h2 id="confirm-title" class="font-serif text-xl">
        {confirmation.question.title}
      </h2>
      <p id="confirm-message" class="mt-2 leading-relaxed">
        {confirmation.question.message}
      </p>
    </div>
    <div class="flex justify-end gap-2 border-t border-rule px-6 py-3">
      <button
        type="button"
        class="min-h-10 rounded px-4 text-sm text-muted hover:text-ink"
        onclick={() => answer(false)}
      >
        Cancel
      </button>
      <button
        type="button"
        class="min-h-10 rounded bg-ink px-4 text-sm font-medium text-surface"
        onclick={() => answer(true)}
      >
        {confirmation.question.confirmLabel}
      </button>
    </div>
  {/if}
</dialog>
