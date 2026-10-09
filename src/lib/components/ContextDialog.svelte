<script lang="ts">
  import type { Post, Uuid } from "../types";
  import { excerpt } from "../stream";
  import { closeContext, contextView } from "../stores/contextView.svelte";

  let {
    postsById,
    nameOf,
  }: { postsById: Map<Uuid, Post>; nameOf: (id: Uuid) => string } = $props();

  let dialog: HTMLDialogElement | undefined = $state();

  $effect(() => {
    if (contextView.open && !dialog?.open) dialog?.showModal();
    if (!contextView.open && dialog?.open) dialog.close();
  });

  const reasons = {
    deleted: "deleted",
    unfinished: "an unfinished or failed reply",
    trimmed: "left out to fit",
  };
  const number = (n: number) => n.toLocaleString();
  const fileReasons = {
    unsupported: "this model cannot read this kind of file",
    too_large: "too large for this model",
    missing: "not available on this device",
  };
</script>

<dialog
  bind:this={dialog}
  aria-labelledby="context-title"
  class="m-auto w-[min(44rem,calc(100vw-1rem))] rounded-lg border border-rule bg-surface p-0 text-ink shadow-xl backdrop:bg-black/30"
  onclose={closeContext}
>
  <div class="max-h-[min(80vh,48rem)] overflow-y-auto px-6 pt-5 pb-4">
    <h2 id="context-title" class="font-serif text-xl">{contextView.title}</h2>

    {#if contextView.loading}
      <p class="mt-3 text-sm text-muted">Working it out…</p>
    {:else if contextView.error}
      <p class="mt-3 text-sm" role="alert">{contextView.error}</p>
    {:else if contextView.sent}
      {@const sent = contextView.sent}
      <p class="mt-2 text-sm text-muted">
        About {number(sent.estimatedTokens)} tokens (an estimate). Up to
        {number(sent.budgetTokens)} are sent before the oldest middle posts are left
        out. Model: {sent.model}.
      </p>

      <details class="mt-3 text-sm">
        <summary class="cursor-pointer text-muted"
          >Instructions to the model</summary
        >
        <p class="mt-1 border-l-2 border-rule pl-3 leading-relaxed text-muted">
          {sent.instructions}
        </p>
      </details>

      <ol class="mt-4 space-y-3" aria-label="Messages sent, in order">
        {#each sent.messages as message, i (i)}
          <li
            class={[
              "rounded border px-3 py-2",
              message.source === "context"
                ? "border-accent/50 bg-accent-soft/40"
                : "border-rule",
            ]}
          >
            <p class="text-xs font-medium text-muted">
              {message.source === "context"
                ? "Context from another branch"
                : message.role === "assistant"
                  ? "Reply chain: the model's own earlier post"
                  : "Reply chain"}
            </p>
            <p
              class="mt-1 font-serif text-[0.9375rem] leading-relaxed whitespace-pre-wrap"
            >
              {message.text}
            </p>
            {#if message.attachments && message.attachments.some((a) => a.delivery !== "text")}
              <p class="mt-1 text-xs text-muted">
                Sent as files: {message.attachments
                  .filter((a) => a.delivery !== "text")
                  .map(
                    (a) =>
                      `${a.filename} (${a.delivery === "pdf" ? "PDF" : "image"})`,
                  )
                  .join(", ")}
              </p>
            {/if}
          </li>
        {/each}
      </ol>

      {#if sent.omittedAttachments && sent.omittedAttachments.length > 0}
        <section class="mt-4" aria-label="Files not sent">
          <h3 class="text-sm font-medium">Files not sent</h3>
          <ul class="mt-1 space-y-1 text-sm text-muted">
            {#each sent.omittedAttachments as file (file.id)}
              <li>{file.filename} ({fileReasons[file.reason]})</li>
            {/each}
          </ul>
        </section>
      {/if}

      {#if sent.omitted.length > 0}
        <section class="mt-4" aria-label="Left out">
          <h3 class="text-sm font-medium">Left out</h3>
          <ul class="mt-1 space-y-1 text-sm text-muted">
            {#each sent.omitted as item (item.postId)}
              {@const post = postsById.get(item.postId)}
              <li>
                {post
                  ? `${nameOf(post.authorId)}: ${excerpt(post.body, 60) || "deleted post"}`
                  : "A post"}
                ({reasons[item.reason]})
              </li>
            {/each}
          </ul>
        </section>
      {/if}
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
