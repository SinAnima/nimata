<script lang="ts">
  import { tick } from "svelte";
  import type { Participant, Post, Revision } from "../types";
  import { authorWallClock, friendlyTime, utcIso } from "../time";
  import { excerpt, paragraphs } from "../stream";
  import { isSubmitShortcut, MODIFIER_LABEL } from "../keys";
  import { errorMessage, postRevisions } from "../api";
  import ParticipantMark from "./ParticipantMark.svelte";

  interface Props {
    post: Post;
    author: Participant | undefined;
    parent: Post | undefined;
    parentAuthorName: string;
    quoteParent: boolean;
    now: number;
    flashing: boolean;
    isReplyTarget: boolean;
    /** Written by the person using this device, so it can be edited. */
    isMine: boolean;
    editing: boolean;
    onReply: () => void;
    onShowParent: () => void;
    onStartEdit: () => void;
    onCancelEdit: () => void;
    onSaveEdit: (body: string) => Promise<boolean>;
    onDelete: () => void;
  }

  let {
    post,
    author,
    parent,
    parentAuthorName,
    quoteParent,
    now,
    flashing,
    isReplyTarget,
    isMine,
    editing,
    onReply,
    onShowParent,
    onStartEdit,
    onCancelEdit,
    onSaveEdit,
    onDelete,
  }: Props = $props();

  let showDetails = $state(false);
  let revisions: Revision[] | null = $state(null);
  let revisionsError: string | null = $state(null);
  let editText = $state("");
  let saving = $state(false);
  let editor: HTMLTextAreaElement | undefined = $state();

  const name = $derived(author?.displayName ?? "Unknown participant");
  const exact = $derived(authorWallClock(post.createdAt, post.tzOffsetMinutes));
  const deleted = $derived(post.deletedAt !== null);
  const fullTime = (at: number) =>
    new Date(at).toLocaleString(undefined, {
      dateStyle: "full",
      timeStyle: "long",
    });

  // Start each edit from the current text, with the cursor in the editor.
  $effect(() => {
    if (!editing) return;
    editText = post.body;
    void tick().then(() => editor?.focus());
  });

  async function toggleDetails(): Promise<void> {
    showDetails = !showDetails;
    if (showDetails && post.editedAt !== null && !deleted) {
      try {
        revisions = await postRevisions(post.id);
        revisionsError = null;
      } catch (e) {
        revisionsError = errorMessage(e);
      }
    }
  }

  async function save(): Promise<void> {
    if (saving || editText.trim() === "") return;
    saving = true;
    const saved = await onSaveEdit(editText);
    saving = false;
    if (saved) revisions = null;
  }

  function onEditorKeydown(event: KeyboardEvent): void {
    if (isSubmitShortcut(event)) {
      event.preventDefault();
      void save();
    } else if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      onCancelEdit();
    }
  }
</script>

<article
  id="post-{post.id}"
  data-post-id={post.id}
  tabindex="-1"
  aria-label="{name}, {friendlyTime(post.createdAt, now)}{deleted
    ? ', deleted'
    : ''}"
  class={[
    "scroll-my-4 border-b border-rule px-5 pt-3.5 pb-4 transition-colors duration-700 outline-none focus:bg-accent-soft/50 sm:px-8",
    flashing && "bg-flash duration-150",
    isReplyTarget && "shadow-[inset_3px_0_0_var(--accent)]",
  ]}
>
  <header class="flex items-baseline gap-2">
    <ParticipantMark kind={author?.kind ?? "human"} />
    <span class="text-[0.9375rem] font-semibold">{name}</span>
    <span class="ml-auto"></span>
    {#if !deleted && !editing}
      <button
        type="button"
        class="-my-2 min-h-9 rounded px-2 text-sm text-muted hover:text-accent"
        aria-label="Reply to {name}"
        onclick={onReply}
      >
        Reply
      </button>
      {#if isMine && post.status === "complete"}
        <button
          type="button"
          class="-my-2 min-h-9 rounded px-2 text-sm text-muted hover:text-accent"
          aria-label="Edit this post"
          onclick={onStartEdit}
        >
          Edit
        </button>
      {/if}
    {/if}
    <button
      type="button"
      class="-my-2 min-h-9 rounded px-1 text-sm text-muted tabular-nums hover:text-ink"
      aria-expanded={showDetails}
      aria-label="Show details"
      title={exact}
      onclick={toggleDetails}
    >
      {#if post.editedAt !== null && !deleted}
        <span class="mr-1 text-xs">Edited</span>
      {/if}
      <time datetime={utcIso(post.createdAt)}>
        {friendlyTime(post.createdAt, now)}
      </time>
    </button>
  </header>

  {#if showDetails}
    <dl
      class="mt-2 grid grid-cols-[auto_1fr] gap-x-4 gap-y-0.5 text-sm text-muted"
    >
      <dt>Written</dt>
      <dd class="tabular-nums">{exact}</dd>
      <dt>Your time</dt>
      <dd class="tabular-nums">{fullTime(post.createdAt)}</dd>
      {#if post.editedAt !== null && !deleted}
        <dt>Edited</dt>
        <dd class="tabular-nums">{fullTime(post.editedAt)}</dd>
      {/if}
      {#if post.deletedAt !== null}
        <dt>Deleted</dt>
        <dd class="tabular-nums">{fullTime(post.deletedAt)}</dd>
      {/if}
      {#if author?.kind === "model"}
        <dt>Model</dt>
        <dd>{author.provider} / {author.model}</dd>
      {/if}
    </dl>

    {#if revisionsError}
      <p class="mt-2 text-sm" role="alert">{revisionsError}</p>
    {:else if revisions && revisions.length > 0}
      <section class="mt-3" aria-label="Earlier versions">
        <h3 class="text-sm font-medium text-muted">Earlier versions</h3>
        <ol class="mt-1 space-y-2">
          {#each revisions as revision (revision.replacedAt)}
            <li class="border-l-2 border-rule pl-3">
              <p class="text-xs text-muted tabular-nums">
                {fullTime(revision.writtenAt)}
              </p>
              <div
                class="max-w-[68ch] space-y-2 font-serif leading-[1.6] text-muted"
              >
                {#each paragraphs(revision.body) as paragraph, i (i)}
                  <p>{paragraph}</p>
                {/each}
              </div>
            </li>
          {/each}
        </ol>
      </section>
    {/if}

    {#if isMine && !deleted}
      <button
        type="button"
        class="mt-2 -ml-2 min-h-9 rounded px-2 text-sm text-muted hover:text-ink"
        onclick={onDelete}
      >
        Delete post
      </button>
    {/if}
  {/if}

  {#if parent}
    <button
      type="button"
      class="mt-1.5 block max-w-full rounded text-left text-sm text-muted hover:text-ink"
      onclick={onShowParent}
    >
      Replying to <span class="font-medium text-accent">{parentAuthorName}</span
      >{#if parent.deletedAt !== null}, deleted post{/if}
      {#if quoteParent && parent.deletedAt === null}
        <span
          class="mt-1 block truncate border-l-2 border-rule pl-2 font-serif italic"
        >
          {excerpt(parent.body)}
        </span>
      {/if}
    </button>
  {/if}

  {#if editing}
    <form
      class="mt-2 max-w-[68ch]"
      aria-label="Edit post"
      onsubmit={(e) => {
        e.preventDefault();
        void save();
      }}
    >
      <label>
        <span class="sr-only">Post text</span>
        <textarea
          bind:this={editor}
          bind:value={editText}
          rows="4"
          class="block w-full resize-y rounded border border-accent bg-paper px-3 py-2 font-serif text-[1.0625rem] leading-[1.6] focus:outline-none"
          onkeydown={onEditorKeydown}></textarea>
      </label>
      <div class="mt-2 flex items-center gap-2">
        <button
          type="submit"
          class="min-h-9 rounded bg-accent px-3 text-sm font-medium text-surface disabled:opacity-45"
          disabled={saving || editText.trim() === ""}
          title="Save ({MODIFIER_LABEL}Enter)"
        >
          Save
        </button>
        <button
          type="button"
          class="min-h-9 rounded px-3 text-sm text-muted hover:text-ink"
          onclick={onCancelEdit}
        >
          Cancel
        </button>
        <span class="text-xs text-muted">
          The current text is kept as an earlier version.
        </span>
      </div>
    </form>
  {:else if deleted}
    <p class="mt-2 font-serif text-[1.0625rem] text-muted italic">
      This post was deleted.
    </p>
  {:else}
    <div
      class="mt-2 max-w-[68ch] space-y-3 font-serif text-[1.0625rem] leading-[1.6]"
    >
      {#each paragraphs(post.body) as paragraph, i (i)}
        <p>{paragraph}</p>
      {/each}
    </div>
  {/if}
</article>
