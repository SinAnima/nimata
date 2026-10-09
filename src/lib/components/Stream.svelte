<script lang="ts">
  import { tick } from "svelte";
  import type { Attachment, DiscussionView, Post, Uuid } from "../types";
  import {
    authorName,
    chronological,
    indexById,
    shouldQuoteParent,
  } from "../stream";
  import { clock } from "../stores/clock.svelte";
  import { notebook } from "../stores/notebook.svelte";
  import { search } from "../stores/search.svelte";
  import PostItem from "./PostItem.svelte";
  import Composer from "./Composer.svelte";
  import ContextDialog from "./ContextDialog.svelte";
  import DiscussionTitle from "./DiscussionTitle.svelte";

  let { view, onBack }: { view: DiscussionView; onBack: () => void } = $props();

  const posts = $derived(chronological(view.posts));
  const postsById = $derived(indexById(view.posts));
  const participantsById = $derived(indexById(view.participants));
  const attachmentsByPost = $derived.by(() => {
    const byPost = new Map<Uuid, Attachment[]>();
    for (const a of view.attachments ?? []) {
      byPost.set(a.postId, [...(byPost.get(a.postId) ?? []), a]);
    }
    return byPost;
  });
  const nameOf = (id: Uuid) => authorName(participantsById, id);
  const composerContext = $derived(
    notebook.composers.get(view.discussion.id).context,
  );
  const replyTo = $derived(notebook.composers.get(view.discussion.id).replyTo);
  const archived = $derived(view.discussion.archivedAt !== null);

  let menuOpen = $state(false);

  function menuAction(action: () => Promise<void>): void {
    menuOpen = false;
    void action();
  }

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

  // Show the post a search result pointed to, once it is on screen.
  $effect(() => {
    const id = search.target;
    if (id === null || !postsById.has(id)) return;
    search.target = null;
    void tick().then(() => showPost(id));
  });

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
    <details class="relative ml-auto shrink-0" bind:open={menuOpen}>
      <summary
        class="flex min-h-10 cursor-pointer list-none items-center rounded px-2 text-sm text-muted hover:text-ink [&::-webkit-details-marker]:hidden"
      >
        More
      </summary>
      <div
        class="absolute right-0 z-10 mt-1 w-56 rounded-md border border-rule bg-surface py-1 shadow-lg"
        role="group"
        aria-label="Discussion actions"
      >
        <button
          type="button"
          class="block min-h-10 w-full px-4 text-left text-sm hover:bg-accent-soft"
          onclick={() => menuAction(() => notebook.exportDiscussion())}
        >
          Export as JSON…
        </button>
        <button
          type="button"
          class="block min-h-10 w-full px-4 text-left text-sm hover:bg-accent-soft"
          onclick={() => menuAction(() => notebook.exportMarkdown())}
        >
          Export as Markdown…
        </button>
        <button
          type="button"
          class="block min-h-10 w-full px-4 text-left text-sm hover:bg-accent-soft"
          onclick={() => menuAction(() => notebook.setArchived(!archived))}
        >
          {archived ? "Unarchive" : "Archive"}
        </button>
        <button
          type="button"
          class="block min-h-10 w-full px-4 text-left text-sm hover:bg-accent-soft"
          onclick={() => menuAction(() => notebook.deleteDiscussion())}
        >
          Delete discussion…
        </button>
      </div>
    </details>
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
            attachments={attachmentsByPost.get(post.id)}
            isReplyTarget={replyTo === post.id}
            isMine={post.authorId === notebook.me?.id}
            canDelete={post.authorId === notebook.me?.id ||
              participantsById.get(post.authorId)?.kind === "model"}
            askableModels={notebook.askableModels}
            editing={notebook.editingPostId === post.id}
            onReply={() => reply(post.id)}
            onShowParent={() => parent && showPost(parent.id)}
            onStartEdit={() => (notebook.editingPostId = post.id)}
            onCancelEdit={() => (notebook.editingPostId = null)}
            onSaveEdit={(body) => notebook.editPost(post.id, body)}
            onDelete={() => notebook.deletePost(post.id)}
            onAsk={(participantId) => notebook.askModel(post.id, participantId)}
            onStop={() => notebook.cancelReply(post.id)}
            onRetry={() => notebook.retryReply(post.id)}
            included={composerContext.includes(post.id)}
            onToggleInclude={() =>
              notebook.composers.toggleContext(view.discussion.id, post.id)}
            contextPosts={post.contextIds.flatMap((id) => {
              const ref = postsById.get(id);
              return ref
                ? [
                    {
                      id,
                      authorName: nameOf(ref.authorId),
                      body: ref.body,
                      deleted: ref.deletedAt !== null,
                    },
                  ]
                : [];
            })}
            onShowPost={(id) => showPost(id)}
          />
        </li>
      {/each}
    </ol>
  </div>

  <ContextDialog {postsById} {nameOf} />

  <Composer
    discussionId={view.discussion.id}
    posts={view.posts}
    {postsById}
    {nameOf}
    onPosted={posted}
  />
</section>
