<script lang="ts">
  import { tick } from "svelte";
  import type { DiscussionView, Uuid } from "../types";
  import {
    authorName,
    chronological,
    indexById,
    shouldQuoteParent,
  } from "../stream";
  import { clock } from "../stores/clock.svelte";
  import { composers } from "../stores/composer.svelte";
  import PostItem from "./PostItem.svelte";
  import Composer from "./Composer.svelte";

  let { view, onBack }: { view: DiscussionView; onBack: () => void } = $props();

  const posts = $derived(chronological(view.posts));
  const postsById = $derived(indexById(view.posts));
  const participantsById = $derived(indexById(view.participants));
  const nameOf = (id: Uuid) => authorName(participantsById, id);
  const replyTo = $derived(composers.get(view.discussion.id).replyTo);

  let flashingId: Uuid | null = $state(null);
  let flashTimer: ReturnType<typeof setTimeout> | undefined;

  function showPost(id: Uuid): void {
    const el = document.getElementById(`post-${id}`);
    if (!el) return;
    const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
    el.scrollIntoView({
      block: "center",
      behavior: reduce ? "auto" : "smooth",
    });
    el.focus({ preventScroll: true });
    flashingId = id;
    clearTimeout(flashTimer);
    flashTimer = setTimeout(() => (flashingId = null), 1200);
  }

  async function reply(id: Uuid): Promise<void> {
    composers.setReplyTo(view.discussion.id, id);
    await tick();
    document.querySelector<HTMLTextAreaElement>("form textarea")?.focus();
  }
</script>

<section
  class="flex h-full min-h-0 flex-col bg-surface"
  aria-labelledby="discussion-title"
>
  <header
    class="flex items-center gap-2 border-b border-rule px-3 pt-[max(0.75rem,env(safe-area-inset-top))] pb-3 sm:px-8"
  >
    <button
      type="button"
      class="min-h-10 rounded px-2 text-accent md:hidden"
      onclick={onBack}
    >
      <span aria-hidden="true">‹</span> Discussions
    </button>
    <div class="min-w-0 max-md:hidden">
      <h1 id="discussion-title" class="truncate font-serif text-xl">
        {view.discussion.title}
      </h1>
      <p class="text-sm text-muted">
        {view.posts.length} posts, {view.participants.length} participants
      </p>
    </div>
  </header>

  <div class="min-h-0 flex-1 overflow-y-auto overscroll-contain">
    <h1 class="px-5 pt-4 font-serif text-xl md:hidden">
      {view.discussion.title}
    </h1>
    <ol aria-label="Posts in order written">
      {#each posts as post, i (post.id)}
        {@const parent = post.parentId
          ? postsById.get(post.parentId)
          : undefined}
        <li>
          <PostItem
            {post}
            author={participantsById.get(post.authorId)}
            {parent}
            parentAuthorName={parent ? nameOf(parent.authorId) : ""}
            quoteParent={shouldQuoteParent(post, posts[i - 1])}
            now={clock.now}
            flashing={flashingId === post.id}
            isReplyTarget={replyTo === post.id}
            onReply={() => reply(post.id)}
            onShowParent={() => parent && showPost(parent.id)}
          />
        </li>
      {/each}
    </ol>
  </div>

  <Composer
    discussionId={view.discussion.id}
    {postsById}
    participants={view.participants}
    {nameOf}
  />
</section>
