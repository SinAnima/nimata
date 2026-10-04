<script lang="ts">
  import { onMount } from "svelte";
  import { getDiscussion, listDiscussions } from "./lib/api";
  import type { DiscussionSummary, DiscussionView } from "./lib/types";
  import { nav } from "./lib/stores/nav.svelte";
  import { startClock } from "./lib/stores/clock.svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import Stream from "./lib/components/Stream.svelte";
  import AboutDialog from "./lib/components/AboutDialog.svelte";

  let discussions: DiscussionSummary[] = $state([]);
  let view: DiscussionView | null = $state(null);
  let error: string | null = $state(null);
  let about: AboutDialog | undefined = $state();

  onMount(() => {
    const stopClock = startClock();
    listDiscussions()
      .then((list) => {
        discussions = list;
        const first = list[0];
        if (first && matchMedia("(min-width: 48rem)").matches) {
          nav.selectedId = first.id;
        }
      })
      .catch((e: unknown) => (error = String(e)));
    return () => void stopClock.then((stop) => stop());
  });

  $effect(() => {
    const id = nav.selectedId;
    if (!id) return;
    getDiscussion(id)
      .then((v) => {
        if (nav.selectedId === id) view = v;
      })
      .catch((e: unknown) => (error = String(e)));
  });
</script>

<div class="h-full md:grid md:grid-cols-[19rem_minmax(0,1fr)]">
  <div
    class={["h-full min-h-0", nav.screen !== "discussions" && "max-md:hidden"]}
  >
    <Sidebar
      {discussions}
      selectedId={nav.selectedId}
      onSelect={(id) => nav.open(id)}
      onAbout={() => about?.open()}
    />
  </div>

  <main
    class={["h-full min-h-0", nav.screen !== "discussion" && "max-md:hidden"]}
  >
    {#if error}
      <div class="flex h-full items-center justify-center bg-surface p-8">
        <p role="alert" class="max-w-md text-center">
          Nimata could not load discussions: {error}
        </p>
      </div>
    {:else if view}
      <Stream {view} onBack={() => nav.back()} />
    {:else}
      <div class="flex h-full items-center justify-center bg-surface p-8">
        <p class="max-w-sm text-center font-serif text-lg text-muted">
          Choose a discussion to read it in the order it was written.
        </p>
      </div>
    {/if}
  </main>
</div>

<AboutDialog bind:this={about} />
