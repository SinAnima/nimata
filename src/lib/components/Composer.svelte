<script lang="ts">
  import type { Post, Uuid } from "../types";
  import { excerpt } from "../stream";
  import { isSubmitShortcut, MODIFIER_LABEL } from "../keys";
  import { notebook } from "../stores/notebook.svelte";

  interface Props {
    discussionId: Uuid;
    postsById: Map<Uuid, Post>;
    nameOf: (id: Uuid) => string;
    onPosted: (post: Post) => void;
  }

  let { discussionId, postsById, nameOf, onPosted }: Props = $props();

  const composers = notebook.composers;
  const current = $derived(composers.get(discussionId));
  const target = $derived(
    current.replyTo ? postsById.get(current.replyTo) : undefined,
  );
  const canPost = $derived(current.draft.trim() !== "");
  let posting = $state(false);

  async function submit(): Promise<void> {
    if (!canPost || posting) return;
    posting = true;
    const post = await notebook.post();
    posting = false;
    if (post) onPosted(post);
  }

  function onKeydown(event: KeyboardEvent): void {
    if (isSubmitShortcut(event)) {
      event.preventDefault();
      void submit();
    } else if (event.key === "Escape" && current.replyTo) {
      event.preventDefault();
      composers.setReplyTo(discussionId, null);
    }
  }
</script>

<form
  class="border-t border-rule bg-surface px-5 pt-3 pb-[max(0.75rem,env(safe-area-inset-bottom))] sm:px-8"
  aria-label="Compose a post"
  onsubmit={(e) => {
    e.preventDefault();
    void submit();
  }}
>
  <div class="flex min-h-7 items-center gap-3 text-sm text-muted">
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
    <span class="ml-auto shrink-0 max-sm:hidden">
      Posting as {notebook.me?.displayName ?? "you"}
    </span>
  </div>

  <div class="mt-2 flex items-end gap-3">
    <label class="min-w-0 flex-1">
      <span class="sr-only">Post text</span>
      <textarea
        data-composer
        rows="3"
        class="block max-h-[40vh] min-h-16 w-full resize-y rounded border border-rule bg-paper px-3 py-2 font-serif text-[1.0625rem] leading-normal placeholder:text-muted focus:border-accent focus:outline-none"
        placeholder={target ? "Write a reply" : "Write a post"}
        value={current.draft}
        oninput={(e) => composers.setDraft(discussionId, e.currentTarget.value)}
        onkeydown={onKeydown}></textarea>
    </label>
    <button
      type="submit"
      class="min-h-10 rounded bg-accent px-4 text-sm font-medium text-surface disabled:opacity-45"
      disabled={!canPost || posting}
      title="Post ({MODIFIER_LABEL}Enter)"
    >
      Post
    </button>
  </div>
</form>
