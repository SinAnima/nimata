<script lang="ts">
  import type { DiscussionFilter, DiscussionSummary, Uuid } from "../types";
  import { dayGroup, friendlyTime, type DayGroup } from "../time";
  import { MODIFIER_LABEL } from "../keys";
  import { clock } from "../stores/clock.svelte";
  import { search } from "../stores/search.svelte";
  import SearchResults from "./SearchResults.svelte";

  interface Props {
    discussions: DiscussionSummary[];
    filter: DiscussionFilter;
    selectedId: Uuid | null;
    onSelect: (id: Uuid) => void;
    onNew: () => void;
    onFilter: (filter: DiscussionFilter) => void;
    onSettings: () => void;
  }

  let {
    discussions,
    filter,
    selectedId,
    onSelect,
    onNew,
    onFilter,
    onSettings,
  }: Props = $props();

  /** Recent searches show while the empty search field has focus. */
  let searchFocused = $state(false);
  const showRecent = $derived(
    searchFocused && !search.active && search.recent.length > 0,
  );

  function onSearchKeydown(event: KeyboardEvent): void {
    if (event.key === "Escape") {
      event.preventDefault();
      search.clear();
      (event.currentTarget as HTMLInputElement).blur();
    } else if (event.key === "Enter") {
      event.preventDefault();
      void search.run();
    } else if (event.key === "ArrowDown" && search.results) {
      // Into the results, then Tab or arrows within them.
      event.preventDefault();
      document
        .querySelector<HTMLButtonElement>(
          "[aria-label='Search results'] li button",
        )
        ?.focus();
    }
  }

  const groups = $derived.by(() => {
    const order: DayGroup[] = ["Today", "Yesterday", "Earlier"];
    return order
      .map((name) => ({
        name,
        items: discussions.filter(
          (d) => dayGroup(d.lastActivityAt, clock.now) === name,
        ),
      }))
      .filter((g) => g.items.length > 0);
  });
</script>

<nav
  class="flex h-full min-h-0 flex-col bg-paper md:border-r md:border-rule"
  aria-label="Discussions"
>
  <header
    class="flex items-center gap-1 px-5 pt-[max(1rem,env(safe-area-inset-top))] pb-2"
  >
    <span class="mr-auto font-serif text-2xl tracking-tight">Nimata</span>
    <button
      type="button"
      class="min-h-10 rounded px-2 text-sm text-muted hover:text-ink"
      onclick={onSettings}
    >
      Settings
    </button>
    <button
      type="button"
      class="min-h-10 rounded px-2 text-sm font-medium text-accent hover:bg-accent-soft"
      title="New discussion ({MODIFIER_LABEL}N)"
      onclick={onNew}
    >
      New
    </button>
  </header>

  <div class="px-5 pb-2" role="search">
    <input
      type="search"
      data-search
      class="block min-h-10 w-full rounded border border-rule bg-surface px-3 text-base placeholder:text-muted focus:border-accent focus:outline-none"
      placeholder="Search everything"
      aria-label="Search all discussions"
      title="Search all discussions ({MODIFIER_LABEL}F)"
      autocomplete="off"
      autocapitalize="off"
      spellcheck="false"
      value={search.query}
      oninput={(e) => search.type(e.currentTarget.value)}
      onkeydown={onSearchKeydown}
      onfocus={() => {
        searchFocused = true;
        void search.loadRecent();
      }}
      onblur={() => (searchFocused = false)}
    />
  </div>

  {#if search.active}
    <div class="flex items-center justify-between px-5">
      <span class="text-sm text-muted" aria-live="polite">
        {#if search.results}
          {search.results.posts.length + search.results.discussions.length === 0
            ? "No matches"
            : `${search.results.posts.length}${search.results.morePosts ? "+" : ""} posts, ${search.results.discussions.length} discussions`}
        {:else}
          Searching…
        {/if}
      </span>
      <button
        type="button"
        class="min-h-10 rounded px-1 text-sm text-accent"
        onclick={() => {
          search.clear();
          document.querySelector<HTMLInputElement>("[data-search]")?.blur();
        }}
      >
        Close search
      </button>
    </div>
  {:else if showRecent}
    <section aria-labelledby="recent-searches" class="pb-1">
      <div class="flex items-center justify-between px-5">
        <h2 id="recent-searches" class="text-sm font-medium text-muted">
          Recent searches
        </h2>
        <button
          type="button"
          class="min-h-10 rounded px-1 text-sm text-muted hover:text-ink"
          onpointerdown={(e) => e.preventDefault()}
          onclick={() => search.clearRecent()}
        >
          Clear
        </button>
      </div>
      <ul>
        {#each search.recent as query (query)}
          <li>
            <button
              type="button"
              class="block min-h-10 w-full truncate px-5 text-left text-sm hover:bg-accent-soft/60"
              onpointerdown={(e) => e.preventDefault()}
              onclick={() => search.useRecent(query)}
            >
              {query}
            </button>
          </li>
        {/each}
      </ul>
    </section>
  {/if}

  {#if filter === "archived" && !search.active}
    <div class="flex items-center justify-between px-5 pb-1">
      <h2 class="text-sm font-medium">Archived</h2>
      <button
        type="button"
        class="min-h-10 rounded px-1 text-sm text-accent"
        onclick={() => onFilter("active")}
      >
        Back to discussions
      </button>
    </div>
  {/if}

  <div class="min-h-0 flex-1 overflow-y-auto overscroll-contain pb-2">
    {#if search.active}
      {#if search.results}
        <SearchResults results={search.results} {selectedId} />
      {/if}
    {:else if showRecent}
      <!-- Recent searches are shown above. -->
    {:else if discussions.length === 0}
      <div class="px-5 pt-6">
        {#if filter === "active"}
          <p class="font-serif text-lg">No discussions yet.</p>
          <p class="mt-1 text-sm text-muted">
            Start one to think something through. Everything stays on this
            device.
          </p>
          <button
            type="button"
            class="mt-4 min-h-10 rounded bg-accent px-4 text-sm font-medium text-surface"
            onclick={onNew}
          >
            Start a discussion
          </button>
        {:else}
          <p class="text-sm text-muted">Nothing is archived.</p>
        {/if}
      </div>
    {/if}

    {#each search.active || showRecent ? [] : groups as group (group.name)}
      <section aria-labelledby="group-{group.name}">
        <h2
          id="group-{group.name}"
          class="px-5 pt-4 pb-1 text-sm font-medium text-muted"
        >
          {group.name}
        </h2>
        <ul>
          {#each group.items as d (d.id)}
            <li>
              <button
                type="button"
                class={[
                  "block w-full px-5 py-2.5 text-left hover:bg-accent-soft/60",
                  d.id === selectedId && "bg-accent-soft",
                ]}
                aria-current={d.id === selectedId ? "true" : undefined}
                onclick={() => onSelect(d.id)}
              >
                <span class="block leading-snug font-medium">{d.title}</span>
                <span class="mt-0.5 line-clamp-2 text-sm text-muted">
                  {d.excerpt}
                </span>
                <span class="mt-1 block text-xs text-muted tabular-nums">
                  {friendlyTime(d.lastActivityAt, clock.now)}
                </span>
              </button>
            </li>
          {/each}
        </ul>
      </section>
    {/each}
  </div>

  {#if filter === "active" && !search.active}
    <footer
      class="border-t border-rule px-5 pt-1 pb-[max(0.25rem,env(safe-area-inset-bottom))]"
    >
      <button
        type="button"
        class="min-h-10 rounded text-sm text-muted hover:text-ink"
        onclick={() => onFilter("archived")}
      >
        Archived discussions
      </button>
    </footer>
  {/if}
</nav>
