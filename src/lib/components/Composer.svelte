<script lang="ts">
  import type { Participant, Post, Uuid } from "../types";
  import { excerpt } from "../stream";
  import { composers } from "../stores/composer.svelte";

  interface Props {
    discussionId: Uuid;
    postsById: Map<Uuid, Post>;
    participants: Participant[];
    nameOf: (id: Uuid) => string;
  }

  let { discussionId, postsById, participants, nameOf }: Props = $props();

  const state = $derived(composers.get(discussionId));
  const target = $derived(
    state.replyTo ? postsById.get(state.replyTo) : undefined,
  );
  const me = $derived(participants.find((p) => p.kind === "human"));
  const models = $derived(participants.filter((p) => p.kind === "model"));

  function onKeydown(event: KeyboardEvent): void {
    if (event.key === "Escape" && state.replyTo) {
      event.preventDefault();
      composers.setReplyTo(discussionId, null);
    }
  }
</script>

<form
  class="border-t border-rule bg-surface px-5 pt-3 pb-[max(0.75rem,env(safe-area-inset-bottom))] sm:px-8"
  aria-label="Compose a post"
  onsubmit={(e) => e.preventDefault()}
>
  <div class="flex items-center gap-3 text-sm text-muted">
    {#if target}
      <span class="min-w-0 truncate">
        Replying to
        <span class="font-medium text-ink">{nameOf(target.authorId)}</span>:
        <span class="font-serif italic">{excerpt(target.body, 60)}</span>
      </span>
      <button
        type="button"
        class="shrink-0 rounded px-1 hover:text-ink"
        onclick={() => composers.setReplyTo(discussionId, null)}
      >
        Cancel reply
      </button>
    {:else}
      <span>New thread</span>
    {/if}

    <label class="ml-auto flex shrink-0 items-center gap-2">
      <span class="sr-only">Author</span>
      <select
        class="rounded border border-rule bg-transparent px-2 py-1 text-ink disabled:opacity-60"
        disabled
      >
        <option>Post as {me?.displayName ?? "me"}</option>
        {#each models as model (model.id)}
          <option>Ask {model.displayName}</option>
        {/each}
      </select>
    </label>
  </div>

  <div class="mt-2 flex items-end gap-3">
    <label class="min-w-0 flex-1">
      <span class="sr-only">Post text</span>
      <textarea
        rows="2"
        class="block w-full resize-none rounded border border-rule bg-paper px-3 py-2 font-serif text-[1.0625rem] leading-normal placeholder:text-muted focus:border-accent focus:outline-none"
        placeholder="Write a post"
        value={state.draft}
        oninput={(e) => composers.setDraft(discussionId, e.currentTarget.value)}
        onkeydown={onKeydown}></textarea>
    </label>
    <button
      type="submit"
      class="min-h-10 rounded bg-accent px-4 text-sm font-medium text-surface disabled:opacity-45"
      disabled
    >
      Post
    </button>
  </div>
  <p class="mt-1.5 text-xs text-muted">
    These are sample discussions. Posting is not available yet.
  </p>
</form>
