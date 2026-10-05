<script lang="ts">
  import type { Participant, Post } from "../types";
  import { authorWallClock, friendlyTime, utcIso } from "../time";
  import { excerpt, paragraphs } from "../stream";
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
    onReply: () => void;
    onShowParent: () => void;
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
    onReply,
    onShowParent,
  }: Props = $props();

  let showDetails = $state(false);
  const name = $derived(author?.displayName ?? "Unknown participant");
  const exact = $derived(authorWallClock(post.createdAt, post.tzOffsetMinutes));
</script>

<article
  id="post-{post.id}"
  data-post-id={post.id}
  tabindex="-1"
  aria-label="{name}, {friendlyTime(post.createdAt, now)}"
  class={[
    "scroll-my-4 border-b border-rule px-5 pt-3.5 pb-4 transition-colors duration-700 outline-none focus:bg-accent-soft/50 sm:px-8",
    flashing && "bg-flash duration-150",
    isReplyTarget && "shadow-[inset_3px_0_0_var(--accent)]",
  ]}
>
  <header class="flex items-baseline gap-2">
    <ParticipantMark kind={author?.kind ?? "human"} />
    <span class="text-[0.9375rem] font-semibold">{name}</span>
    <button
      type="button"
      class="ml-auto -my-2 min-h-9 rounded px-2 text-sm text-muted hover:text-accent"
      aria-label="Reply to {name}"
      onclick={onReply}
    >
      Reply
    </button>
    <button
      type="button"
      class="-my-2 min-h-9 rounded px-1 text-sm text-muted tabular-nums hover:text-ink"
      aria-expanded={showDetails}
      aria-label="Show exact time"
      title={exact}
      onclick={() => (showDetails = !showDetails)}
    >
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
      <dd class="tabular-nums">
        {new Date(post.createdAt).toLocaleString(undefined, {
          dateStyle: "full",
          timeStyle: "long",
        })}
      </dd>
      {#if author?.kind === "model"}
        <dt>Model</dt>
        <dd>{author.provider} / {author.model}</dd>
      {/if}
    </dl>
  {/if}

  {#if parent}
    <button
      type="button"
      class="mt-1.5 block max-w-full rounded text-left text-sm text-muted hover:text-ink"
      onclick={onShowParent}
    >
      Replying to <span class="font-medium text-accent">{parentAuthorName}</span
      >
      {#if quoteParent}
        <span
          class="mt-1 block truncate border-l-2 border-rule pl-2 font-serif italic"
        >
          {excerpt(parent.body)}
        </span>
      {/if}
    </button>
  {/if}

  <div
    class="mt-2 max-w-[68ch] space-y-3 font-serif text-[1.0625rem] leading-[1.6]"
  >
    {#each paragraphs(post.body) as paragraph, i (i)}
      <p>{paragraph}</p>
    {/each}
  </div>
</article>
