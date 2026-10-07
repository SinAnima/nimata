<script lang="ts">
  import type { SearchResults, Uuid } from "../types";
  import { friendlyTime } from "../time";
  import { markedParts } from "../search";
  import { clock } from "../stores/clock.svelte";
  import { search } from "../stores/search.svelte";

  let {
    results,
    selectedId,
  }: { results: SearchResults; selectedId: Uuid | null } = $props();

  const nothing = $derived(
    results.posts.length === 0 && results.discussions.length === 0,
  );
</script>

{#snippet marked(text: string)}
  {#each markedParts(text) as part, i (i)}
    {#if part.match}<mark class="rounded-sm bg-flash text-ink">{part.text}</mark
      >{:else}{part.text}{/if}
  {/each}
{/snippet}

<div aria-label="Search results" role="region">
  {#each results.warnings as warning (warning)}
    <p role="status" class="mx-5 mt-3 text-sm text-muted">{warning}</p>
  {/each}

  {#if nothing}
    <div class="px-5 pt-4">
      <p class="font-serif text-lg">Nothing matches.</p>
      <p class="mt-1 text-sm text-muted">
        Words match their beginnings and ignore accents. Archived discussions
        are left out unless you add <code>in:archived</code>.
      </p>
    </div>
  {/if}

  {#if results.discussions.length > 0}
    <section aria-labelledby="search-discussions">
      <h2
        id="search-discussions"
        class="px-5 pt-4 pb-1 text-sm font-medium text-muted"
      >
        Discussions
      </h2>
      <ul>
        {#each results.discussions as hit (hit.discussionId)}
          <li>
            <button
              type="button"
              class={[
                "block w-full px-5 py-2 text-left hover:bg-accent-soft/60",
                hit.discussionId === selectedId && "bg-accent-soft",
              ]}
              onclick={() => search.openDiscussion(hit)}
            >
              <span class="block leading-snug font-medium"
                >{@render marked(hit.title)}</span
              >
              <span class="mt-0.5 block text-xs text-muted tabular-nums">
                {hit.archived ? "Archived, " : ""}{friendlyTime(
                  hit.lastActivityAt,
                  clock.now,
                )}
              </span>
            </button>
          </li>
        {/each}
      </ul>
    </section>
  {/if}

  {#if results.posts.length > 0}
    <section aria-labelledby="search-posts">
      <h2
        id="search-posts"
        class="px-5 pt-4 pb-1 text-sm font-medium text-muted"
      >
        Posts
      </h2>
      <ul>
        {#each results.posts as hit (hit.postId)}
          <li>
            <button
              type="button"
              class="block w-full px-5 py-2.5 text-left hover:bg-accent-soft/60"
              onclick={() => search.openPost(hit)}
            >
              <span class="block truncate text-xs text-muted">
                {hit.discussionTitle}
              </span>
              <span class="mt-1 line-clamp-3 font-serif leading-snug"
                >{@render marked(hit.snippet)}</span
              >
              <span class="mt-1 block text-xs text-muted tabular-nums">
                <span class="font-medium">{hit.authorName}</span>,
                {friendlyTime(hit.createdAt, clock.now)}
              </span>
            </button>
          </li>
        {/each}
      </ul>
      {#if results.morePosts}
        <p class="px-5 pt-2 text-sm text-muted">
          Showing the best {results.posts.length} matches. Add words or a filter to
          narrow them.
        </p>
      {/if}
    </section>
  {/if}

  <details class="mx-5 mt-5 text-sm text-muted">
    <summary class="min-h-10 cursor-pointer content-center">Search tips</summary
    >
    <dl class="mt-1 grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 pb-2">
      <dt><code>"exact words"</code></dt>
      <dd>a phrase</dd>
      <dt><code>from:claude</code></dt>
      <dd>by a model or person; <code>from:me</code> is you</dd>
      <dt><code>after:2026-09-01</code></dt>
      <dd>on or after a day; also <code>before:</code> and <code>on:</code></dd>
      <dt><code>discussion:"assets"</code></dt>
      <dd>in discussions whose title contains this</dd>
      <dt><code>in:archived</code></dt>
      <dd>include archived discussions</dd>
    </dl>
  </details>
</div>
