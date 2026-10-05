<script lang="ts">
  import { tick } from "svelte";
  import type { DiscussionView, Post, Uuid } from "../types";
  import {
    authorName,
    chronological,
    indexById,
    shouldQuoteParent,
  } from "../stream";
  import { clock } from "../stores/clock.svelte";
  import { notebook } from "../stores/notebook.svelte";
  import PostItem from "./PostItem.svelte";
  import Composer from "./Composer.svelte";
  import DiscussionTitle from "./DiscussionTitle.svelte";

  let { view, onBack }: { view: DiscussionView; onBack: () => void } = $props();

  const posts = $derived(chronological(view.posts));
  const postsById = $derived(indexById(view.posts));
  const participantsById = $derived(indexById(view.participants));
  const nameOf = (id: Uuid) => authorName(participantsById, id);
  const replyTo = $derived(notebook.composers.get(view.discussion.id).replyTo);
  const archived = $derived(view.discussion.archivedAt !== null);

  let flashingId: Uuid | null = $state(null);
  let flashTimer: ReturnType<typeof setTimeout> | undefined;

  function showPost(id: Uuid, focus = true): void {
    const el = document.getElementById(`post-${id}`);
    if (!el) return;
    const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
    el.scrollIntoView({
      block: "nearest",
      behavior: reduce ? "auto" : "smooth",
    });
    if (focus) el.focus({ preventScroll: true });
    flashingId = id;
    clearTimeout(flashTimer);
    flashTimer = setTimeout(() => (flashingId = null), 1200);
  }

  async function reply(id: Uuid): Promise<void> {
    notebook.composers.setReplyTo(view.discussion.id, id);
    await tick();
    document.querySelector<HTMLTextAreaElement>("[data-composer]")?.focus();
  }

  async function posted(post: Post): Promise<void> {
    await tick();
    showPost(post.id, false);
  }
</script>

<section
  class="flex h-full min-h-0 flex-col bg-surface"
  aria-label={view.discussion.title}
>
  <header
    class="flex items-center gap-3 border-b border-rule px-3 pt-[max(0.75rem,env(safe-area-inset-top))] pb-3 sm:px-8"
  >
    <button
      type="button"
      class="min-h-10 rounded px-2 text-accent md:hidden"
      onclick={onBack}
    >
      <span aria-hidden="true">‹</span> Discussions
    </button>
    <div class="min-w-0 flex-1 max-md:hidden">
      <DiscussionTitle title={view.discussion.title} />
      <p class="text-sm text-muted">
        {view.posts.length}
        {view.posts.length === 1 ? "post" : "posts"}{archived
          ? ", archived"
          : ""}
      </p>
    </div>
    <button
      type="button"
      class="ml-auto min-h-10 shrink-0 rounded px-2 text-sm text-muted hover:text-ink"
      onclick={() => notebook.setArchived(!archived)}
    >
      {archived ? "Unarchive" : "Archive"}
    </button>
  </header>

  <div class="min-h-0 flex-1 overflow-y-auto overscroll-contain">
    <div class="px-5 pt-4 md:hidden">
      <DiscussionTitle title={view.discussion.title} />
      {#if archived}<p class="text-sm text-muted">Archived</p>{/if}
    </div>
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
    {nameOf}
    onPosted={posted}
  />
</section>
