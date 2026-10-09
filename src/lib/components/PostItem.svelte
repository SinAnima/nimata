<script lang="ts">
  import { tick } from "svelte";
  import type {
    Attachment,
    Generation,
    ModelParticipant,
    Participant,
    Post,
    Revision,
  } from "../types";
  import { authorWallClock, friendlyTime, utcIso } from "../time";
  import { excerpt, paragraphs } from "../stream";
  import { isSubmitShortcut, MODIFIER_LABEL } from "../keys";
  import { errorMessage, postRevisions, replyDetails } from "../api";
  import { showContext } from "../stores/contextView.svelte";
  import { search } from "../stores/search.svelte";
  import { highlightParts } from "../search";
  import ParticipantMark from "./ParticipantMark.svelte";
  import AttachmentList from "./AttachmentList.svelte";

  interface Props {
    post: Post;
    /** Files attached to the post. */
    attachments?: Attachment[];
    author: Participant | undefined;
    parent: Post | undefined;
    parentAuthorName: string;
    quoteParent: boolean;
    now: number;
    flashing: boolean;
    isReplyTarget: boolean;
    /** Written by the person using this device, so it can be edited. */
    isMine: boolean;
    /** Theirs, or a model's reply in their discussion. */
    canDelete: boolean;
    /** Models that can be asked to reply to this post. */
    askableModels: ModelParticipant[];
    editing: boolean;
    onReply: () => void;
    onShowParent: () => void;
    onStartEdit: () => void;
    onCancelEdit: () => void;
    onSaveEdit: (body: string) => Promise<boolean>;
    onDelete: () => void;
    onAsk: (participantId: string) => void;
    onStop: () => void;
    onRetry: () => void;
    /** Chosen as context for the post being written. */
    included: boolean;
    onToggleInclude: () => void;
    /** Posts this one chose as context. */
    contextPosts: {
      id: string;
      authorName: string;
      body: string;
      deleted: boolean;
    }[];
    onShowPost: (id: string) => void;
  }

  let {
    post,
    author,
    parent,
    parentAuthorName,
    quoteParent,
    now,
    flashing,
    attachments = [],
    isReplyTarget,
    isMine,
    canDelete,
    askableModels,
    editing,
    onReply,
    onShowParent,
    onStartEdit,
    onCancelEdit,
    onSaveEdit,
    onDelete,
    onAsk,
    onStop,
    onRetry,
    included,
    onToggleInclude,
    contextPosts,
    onShowPost,
  }: Props = $props();

  /** Words from the search that opened this discussion. */
  const searchTerms = $derived(
    search.highlightIn === post.discussionId ? search.highlight : [],
  );

  let showDetails = $state(false);
  let revisions: Revision[] | null = $state(null);
  let revisionsError: string | null = $state(null);
  let editText = $state("");
  let saving = $state(false);
  let editor: HTMLTextAreaElement | undefined = $state();
  let askMenuOpen = $state(false);
  /** How a model reply came about; loaded when it is needed. */
  let generation: Generation | null = $state(null);

  const name = $derived(author?.displayName ?? "Unknown participant");
  const exact = $derived(authorWallClock(post.createdAt, post.tzOffsetMinutes));
  const deleted = $derived(post.deletedAt !== null);
  const isModel = $derived(author?.kind === "model");
  const complete = $derived(post.status === "complete");
  const ended = $derived(
    post.status === "failed" || post.status === "cancelled",
  );

  // Why a reply failed is kept with its request record.
  $effect(() => {
    if (post.status === "failed" && !deleted) void loadGeneration();
  });

  async function loadGeneration(): Promise<void> {
    try {
      generation = await replyDetails(post.id);
    } catch {
      generation = null;
    }
  }

  function showSent(): void {
    void showContext(`What ${name} was sent`, async () => {
      return (await replyDetails(post.id))?.sent ?? null;
    });
  }

  function ask(participantId: string): void {
    askMenuOpen = false;
    onAsk(participantId);
  }

  /** Each enabled model answers separately, as sibling replies. */
  function askAll(): void {
    askMenuOpen = false;
    for (const model of askableModels) onAsk(model.participant.id);
  }
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
    if (showDetails && isModel) void loadGeneration();
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
    {#if !deleted && !editing && complete}
      <button
        type="button"
        class="-my-2 min-h-9 rounded px-2 text-sm text-muted hover:text-accent"
        aria-label="Reply to {name}"
        onclick={onReply}
      >
        Reply
      </button>
      {#if askableModels.length === 1}
        {@const model = askableModels[0]!}
        <button
          type="button"
          class="-my-2 min-h-9 rounded px-2 text-sm text-muted hover:text-accent"
          onclick={() => ask(model.participant.id)}
        >
          Ask {model.participant.displayName}
        </button>
      {:else if askableModels.length > 1}
        <details class="relative" bind:open={askMenuOpen}>
          <summary
            class="-my-2 flex min-h-9 cursor-pointer list-none items-center rounded px-2 text-sm text-muted hover:text-accent [&::-webkit-details-marker]:hidden"
          >
            Ask…
          </summary>
          <div
            class="absolute right-0 z-10 mt-1 w-52 rounded-md border border-rule bg-surface py-1 shadow-lg"
            role="group"
            aria-label="Ask a model to reply"
          >
            {#each askableModels as model (model.participant.id)}
              <button
                type="button"
                class="block min-h-10 w-full px-4 text-left text-sm hover:bg-accent-soft"
                onclick={() => ask(model.participant.id)}
              >
                {model.participant.displayName}
              </button>
            {/each}
            <button
              type="button"
              class="block min-h-10 w-full border-t border-rule px-4 text-left text-sm hover:bg-accent-soft"
              onclick={askAll}
            >
              All of them
            </button>
          </div>
        </details>
      {/if}
      <button
        type="button"
        class={[
          "-my-2 min-h-9 rounded px-2 text-sm hover:text-accent",
          included ? "font-medium text-accent" : "text-muted",
        ]}
        aria-pressed={included}
        title={included
          ? "Included as context in the post you are writing"
          : "Include as context in the post you are writing"}
        onclick={onToggleInclude}
      >
        {included ? "Included" : "Include"}
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
        {#if post.providerMetadata?.model}
          <dt>Answered by</dt>
          <dd>{post.providerMetadata.model}</dd>
        {/if}
        {#if post.providerMetadata?.inputTokens != null}
          <dt>Tokens</dt>
          <dd class="tabular-nums">
            {post.providerMetadata.inputTokens} sent, {post.providerMetadata
              .outputTokens} received
          </dd>
        {/if}
        {#if post.providerMetadata?.requestId}
          <dt>Request ID</dt>
          <dd class="break-all">{post.providerMetadata.requestId}</dd>
        {/if}
        {#if generation?.error}
          <dt>Error</dt>
          <dd>{generation.error}</dd>
        {/if}
        {#if generation}
          <dt>Shown</dt>
          <dd>
            {generation.contextPostIds.length}
            {generation.contextPostIds.length === 1 ? "post" : "posts"}
            {#if generation.sent}
              <button
                type="button"
                class="ml-1 rounded font-medium text-accent hover:underline"
                onclick={showSent}
              >
                See exactly what was sent
              </button>
            {/if}
          </dd>
        {/if}
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

    {#if canDelete && !deleted && post.status !== "streaming"}
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

  {#each contextPosts as ref (ref.id)}
    <button
      type="button"
      class="mt-1 block max-w-full rounded text-left text-sm text-muted hover:text-ink"
      onclick={() => onShowPost(ref.id)}
    >
      Also considering <span class="font-medium text-accent"
        >{ref.authorName}</span
      >{#if ref.deleted}, deleted post{:else}:
        <span class="font-serif italic">{excerpt(ref.body, 70)}</span>{/if}
    </button>
  {/each}

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
    {#if post.body !== ""}
      <div
        class="mt-2 max-w-[68ch] space-y-3 font-serif text-[1.0625rem] leading-[1.6]"
      >
        {#each paragraphs(post.body) as paragraph, i (i)}
          <p>
            {#each highlightParts(paragraph, searchTerms) as part, j (j)}
              {#if part.match}<mark class="rounded-sm bg-flash text-ink"
                  >{part.text}</mark
                >{:else}{part.text}{/if}
            {/each}
          </p>
        {/each}
      </div>
    {/if}

    {#if attachments.length > 0}
      <AttachmentList {attachments} />
    {/if}

    {#if post.status === "streaming"}
      <div class="mt-2 flex items-center gap-3 text-sm text-muted">
        <span role="status">
          {post.body === "" ? `Waiting for ${name}…` : "Writing…"}
        </span>
        <button
          type="button"
          class="min-h-9 rounded px-2 hover:text-ink"
          onclick={onStop}
        >
          Stop
        </button>
      </div>
    {:else if ended}
      <div
        class="mt-2 flex max-w-[68ch] flex-wrap items-baseline gap-x-3 gap-y-1 text-sm"
      >
        <p class="text-muted" role={post.status === "failed" ? "alert" : null}>
          {#if post.status === "cancelled"}
            Stopped before finishing.
          {:else}
            This reply failed{generation?.error ? `: ${generation.error}` : "."}
          {/if}
        </p>
        <button
          type="button"
          class="min-h-9 rounded px-2 font-medium text-accent hover:bg-accent-soft"
          onclick={onRetry}
        >
          Retry
        </button>
      </div>
    {:else if post.providerMetadata?.incompleteReason}
      <p class="mt-2 text-sm text-muted">
        {post.providerMetadata.incompleteReason === "max_output_tokens"
          ? "The reply stopped at its length limit."
          : `The reply stopped early (${post.providerMetadata.incompleteReason}).`}
      </p>
    {/if}
  {/if}
</article>
