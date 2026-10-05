<script lang="ts">
  import { onMount, tick } from "svelte";
  import { nav } from "./lib/stores/nav.svelte";
  import { notebook } from "./lib/stores/notebook.svelte";
  import { startClock } from "./lib/stores/clock.svelte";
  import { adjacentPost, hasModifier, isTypingTarget } from "./lib/keys";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import Stream from "./lib/components/Stream.svelte";
  import NewDiscussion from "./lib/components/NewDiscussion.svelte";
  import SettingsDialog from "./lib/components/SettingsDialog.svelte";

  let settings: SettingsDialog | undefined = $state();

  onMount(() => {
    const stopClock = startClock();
    void notebook.init(matchMedia("(min-width: 48rem)").matches);

    // Save unsent drafts before the window is hidden or closed.
    const flush = () => notebook.composers.flush();
    const onVisibility = () => document.hidden && flush();
    document.addEventListener("visibilitychange", onVisibility);
    window.addEventListener("pagehide", flush);
    return () => {
      document.removeEventListener("visibilitychange", onVisibility);
      window.removeEventListener("pagehide", flush);
      void stopClock.then((stop) => stop());
    };
  });

  function focusedPostId(): string | null {
    const el = document.activeElement?.closest<HTMLElement>("[data-post-id]");
    return el?.dataset.postId ?? null;
  }

  async function onKeydown(event: KeyboardEvent): Promise<void> {
    if (hasModifier(event) && event.key.toLowerCase() === "n") {
      event.preventDefault();
      nav.startNew();
      return;
    }
    if (
      isTypingTarget(event.target) ||
      event.metaKey ||
      event.ctrlKey ||
      event.altKey ||
      nav.screen !== "discussion"
    ) {
      return;
    }
    const key = event.key.toLowerCase();
    if (key === "j" || key === "k") {
      event.preventDefault();
      const ids = [
        ...document.querySelectorAll<HTMLElement>("[data-post-id]"),
      ].map((el) => el.dataset.postId ?? "");
      const next = adjacentPost(ids, focusedPostId(), key === "j" ? 1 : -1);
      const el = next ? document.getElementById(`post-${next}`) : null;
      el?.focus({ preventScroll: true });
      el?.scrollIntoView({ block: "nearest" });
    } else if (key === "r" && notebook.view) {
      const id = focusedPostId();
      if (!id) return;
      event.preventDefault();
      notebook.composers.setReplyTo(notebook.view.discussion.id, id);
      await tick();
      document.querySelector<HTMLTextAreaElement>("[data-composer]")?.focus();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if notebook.fatal}
  <div class="flex h-full items-center justify-center bg-surface p-8">
    <div role="alert" class="max-w-lg">
      <h1 class="font-serif text-2xl">Nimata cannot open your discussions</h1>
      <p class="mt-3 leading-relaxed">{notebook.fatal}</p>
      <p class="mt-3 text-sm text-muted">
        Nothing has been changed or deleted. Quit Nimata and check that the
        database file is readable, then reopen it.
      </p>
    </div>
  </div>
{:else}
  <div class="h-full md:grid md:grid-cols-[19rem_minmax(0,1fr)]">
    <div
      class={[
        "h-full min-h-0",
        nav.screen !== "discussions" && "max-md:hidden",
      ]}
    >
      <Sidebar
        discussions={notebook.discussions}
        filter={notebook.filter}
        selectedId={nav.screen === "new" ? null : nav.selectedId}
        onSelect={(id) => notebook.open(id)}
        onNew={() => nav.startNew()}
        onFilter={(f) => notebook.showFilter(f)}
        onSettings={() => settings?.open()}
      />
    </div>

    <main
      class={[
        "flex h-full min-h-0 flex-col",
        nav.screen === "discussions" && "max-md:hidden",
      ]}
    >
      {#if notebook.notice}
        <div
          role="alert"
          class="flex items-start gap-3 border-b border-rule bg-flash px-5 py-2 text-sm sm:px-8"
        >
          <p class="flex-1">{notebook.notice}</p>
          <button
            type="button"
            class="rounded px-1 font-medium"
            onclick={() => (notebook.notice = null)}
          >
            Dismiss
          </button>
        </div>
      {/if}

      <div class="min-h-0 flex-1">
        {#if nav.screen === "new"}
          <NewDiscussion
            onCancel={() =>
              nav.selectedId ? nav.open(nav.selectedId) : nav.back()}
          />
        {:else if notebook.view}
          {#key notebook.view.discussion.id}
            <Stream view={notebook.view} onBack={() => nav.back()} />
          {/key}
        {:else}
          <div class="flex h-full items-center justify-center bg-surface p-8">
            <p class="max-w-sm text-center font-serif text-lg text-muted">
              {notebook.discussions.length > 0
                ? "Choose a discussion to read it in the order it was written."
                : "Start a discussion to begin your notebook."}
            </p>
          </div>
        {/if}
      </div>
    </main>
  </div>
{/if}

<SettingsDialog bind:this={settings} />
