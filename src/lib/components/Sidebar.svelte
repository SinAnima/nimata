<script lang="ts">
  import type { DiscussionSummary, Uuid } from "../types";
  import { dayGroup, friendlyTime, type DayGroup } from "../time";
  import { clock } from "../stores/clock.svelte";

  interface Props {
    discussions: DiscussionSummary[];
    selectedId: Uuid | null;
    onSelect: (id: Uuid) => void;
    onAbout: () => void;
  }

  let { discussions, selectedId, onSelect, onAbout }: Props = $props();

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
    class="flex items-baseline justify-between px-5 pt-[max(1rem,env(safe-area-inset-top))] pb-3"
  >
    <span class="font-serif text-2xl tracking-tight">Nimata</span>
    <button
      type="button"
      class="min-h-10 rounded px-2 text-sm text-muted hover:text-ink"
      onclick={onAbout}
    >
      About
    </button>
  </header>

  <div class="min-h-0 flex-1 overflow-y-auto overscroll-contain pb-4">
    {#each groups as group (group.name)}
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
                <span class="block font-medium leading-snug">{d.title}</span>
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
</nav>
