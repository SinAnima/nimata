<script lang="ts">
  import { tick } from "svelte";
  import type { Post, Uuid } from "../types";
  import { excerpt } from "../stream";
  import { isSubmitShortcut, MODIFIER_LABEL } from "../keys";
  import { defaultTarget, whoAnswers, type AskChoice } from "../compose";
  import { aliasesOf, mentionAt, resolveAlias } from "../mentions";
  import { notebook } from "../stores/notebook.svelte";
  import AskLine from "./AskLine.svelte";
  import { previewContext } from "../api";
  import { showContext } from "../stores/contextView.svelte";
  import { carriesFiles, droppedFiles } from "../attach";
  import StagedFiles from "./StagedFiles.svelte";

  interface Props {
    discussionId: Uuid;
    posts: readonly Post[];
    postsById: Map<Uuid, Post>;
    nameOf: (id: Uuid) => string;
    onPosted: (post: Post) => void;
  }

  let { discussionId, posts, postsById, nameOf, onPosted }: Props = $props();

  const composers = notebook.composers;
  const current = $derived(composers.get(discussionId));
  const latest = $derived(defaultTarget(posts));
  const chosenTarget = $derived(
    current.replyTo ? postsById.get(current.replyTo) : undefined,
  );
  const target = $derived(chosenTarget ?? latest);
  const isLatest = $derived(target?.id === latest?.id);

  let choice: AskChoice = $state(null);
  let cursor = $state(0);
  let textarea: HTMLTextAreaElement | undefined = $state();

  const answerers = $derived(
    whoAnswers({
      text: current.draft,
      choice,
      target,
      posts,
      askable: notebook.askableModels,
      defaultModelId: notebook.defaultModelId,
    }),
  );
  const problems = $derived(answerers.mentions.problems);

  /** Models matching the "@partial" the cursor is in, for completion. */
  const partial = $derived(mentionAt(current.draft, cursor));
  const suggestions = $derived.by(() => {
    if (!partial) return [];
    const matches = notebook.askableModels.filter((m) =>
      aliasesOf(m).some((a) => a.startsWith(partial.alias)),
    );
    const exact = resolveAlias(partial.alias, notebook.askableModels);
    // Nothing to suggest once the alias names one model exactly.
    if (
      exact.kind === "model" &&
      aliasesOf(exact.model).includes(partial.alias)
    )
      return [];
    return matches;
  });

  /** Files still being stored, and whether files are being dragged over. */
  let staging = $state(0);
  let dragging = $state(false);
  let picker: HTMLInputElement | undefined = $state();

  const canPost = $derived(
    (current.draft.trim() !== "" || current.attachments.length > 0) &&
      target !== undefined &&
      problems.length === 0 &&
      staging === 0,
  );

  async function attach(files: File[]): Promise<void> {
    if (files.length === 0) return;
    await notebook.stageFiles(
      files,
      (file) => composers.addAttachment(discussionId, file),
      (remaining) => (staging = remaining),
    );
  }

  function onDragOver(event: DragEvent): void {
    if (!carriesFiles(event)) return;
    event.preventDefault();
    dragging = true;
  }

  function onDrop(event: DragEvent): void {
    if (!carriesFiles(event)) return;
    event.preventDefault();
    dragging = false;
    void attach(droppedFiles(event));
  }
  let posting = $state(false);

  /** The posts chosen as context, for the chips above the text box. */
  const contextPosts = $derived(
    current.context.flatMap((id) => {
      const post = postsById.get(id);
      return post ? [post] : [];
    }),
  );

  function previewFor(): void {
    const model = answerers.models[0];
    if (!model || !target) return;
    const others = answerers.models.length - 1;
    const title =
      others > 0
        ? `What ${model.participant.displayName} will be sent (the ${others} other ${others === 1 ? "model gets" : "models get"} the same posts)`
        : `What ${model.participant.displayName} will be sent`;
    void showContext(title, () =>
      previewContext(
        discussionId,
        target.id,
        model.participant.id,
        current.draft,
        current.context,
        current.attachments,
      ),
    );
  }

  async function submit(): Promise<void> {
    if (!canPost || posting || !target) return;
    posting = true;
    const post = await notebook.post(target.id, answerers.models);
    posting = false;
    if (post) {
      choice = null;
      onPosted(post);
    }
  }

  async function complete(alias: string): Promise<void> {
    if (!partial) return;
    const text = current.draft;
    const before = text.slice(0, partial.start);
    const after = text.slice(partial.end);
    const insert = `@${alias}${after.startsWith(" ") ? "" : " "}`;
    composers.setDraft(discussionId, before + insert + after);
    cursor = before.length + insert.length;
    await tick();
    textarea?.focus();
    textarea?.setSelectionRange(cursor, cursor);
  }

  function onKeydown(event: KeyboardEvent): void {
    if (isSubmitShortcut(event)) {
      event.preventDefault();
      void submit();
    } else if (event.key === "Tab" && suggestions.length === 1) {
      event.preventDefault();
      const [model] = suggestions;
      void complete(
        aliasesOf(model!).find((a) => a.startsWith(partial!.alias))!,
      );
    } else if (event.key === "Escape" && current.replyTo) {
      event.preventDefault();
      composers.setReplyTo(discussionId, null);
    }
  }

  function trackCursor(): void {
    cursor = textarea?.selectionStart ?? 0;
  }
</script>

<form
  class={[
    "border-t border-rule bg-surface px-5 pt-3 pb-[max(0.75rem,env(safe-area-inset-bottom))] sm:px-8",
    dragging &&
      "bg-accent-soft/60 outline-2 outline-accent outline-dashed -outline-offset-4",
  ]}
  aria-label="Compose a post"
  ondragover={onDragOver}
  ondragleave={() => (dragging = false)}
  ondrop={onDrop}
  onsubmit={(e) => {
    e.preventDefault();
    void submit();
  }}
>
  <div class="flex min-h-7 items-center gap-3 text-sm text-muted">
    {#if target}
      <span class="min-w-0 truncate">
        Replying to
        <span class="font-medium text-ink">{nameOf(target.authorId)}</span>:
        <span class="font-serif italic">{excerpt(target.body, 60)}</span>
      </span>
      {#if !isLatest}
        <button
          type="button"
          class="shrink-0 rounded px-1 hover:text-ink"
          onclick={() => composers.setReplyTo(discussionId, null)}
        >
          Reply to latest
        </button>
      {/if}
    {/if}
  </div>

  {#if contextPosts.length > 0}
    <div
      class="mt-1.5 flex flex-wrap items-center gap-1.5 text-sm"
      role="group"
      aria-label="Also considering"
    >
      <span class="text-muted">Also considering</span>
      {#each contextPosts as post (post.id)}
        <span
          class="inline-flex max-w-full items-center gap-1 rounded border border-accent/50 bg-accent-soft/40 py-0.5 pr-1 pl-2"
        >
          <span class="truncate">
            <span class="font-medium">{nameOf(post.authorId)}</span>:
            <span class="font-serif italic">{excerpt(post.body, 40)}</span>
          </span>
          <button
            type="button"
            class="min-h-7 shrink-0 rounded px-1 text-muted hover:text-ink"
            aria-label="Stop considering {nameOf(post.authorId)}'s post"
            onclick={() => composers.toggleContext(discussionId, post.id)}
          >
            ×
          </button>
        </span>
      {/each}
    </div>
  {/if}

  <StagedFiles
    files={current.attachments}
    {staging}
    answerers={answerers.models}
    onRemove={(i) => notebook.removeStaged(discussionId, i)}
  />

  <div class="mt-2 flex items-end gap-3">
    <label class="min-w-0 flex-1">
      <span class="sr-only">Post text</span>
      <textarea
        bind:this={textarea}
        data-composer
        rows="3"
        class="block max-h-[40vh] min-h-16 w-full resize-y rounded border border-rule bg-paper px-3 py-2 font-serif text-[1.0625rem] leading-normal placeholder:text-muted focus:border-accent focus:outline-none"
        placeholder={notebook.askableModels.length > 0
          ? "Write a reply. Mention @someone to ask a particular model."
          : "Write a reply"}
        value={current.draft}
        oninput={(e) => {
          composers.setDraft(discussionId, e.currentTarget.value);
          trackCursor();
        }}
        onkeyup={trackCursor}
        onclick={trackCursor}
        onkeydown={onKeydown}></textarea>
    </label>
    <input
      bind:this={picker}
      type="file"
      multiple
      class="hidden"
      aria-hidden="true"
      tabindex="-1"
      data-attach-input
      onchange={(e) => {
        const files = [...(e.currentTarget.files ?? [])];
        e.currentTarget.value = "";
        void attach(files);
      }}
    />
    <button
      type="button"
      class="min-h-10 rounded border border-rule px-3 text-sm hover:bg-accent-soft"
      title="Attach files (or drop them here)"
      onclick={() => picker?.click()}
    >
      Attach…
    </button>
    <button
      type="submit"
      class="min-h-10 rounded bg-accent px-4 text-sm font-medium text-surface disabled:opacity-45"
      disabled={!canPost || posting}
      title="Post ({MODIFIER_LABEL}Enter)"
    >
      Post
    </button>
  </div>

  {#if suggestions.length > 0}
    <div
      class="mt-1.5 flex flex-wrap items-center gap-1 text-sm"
      role="group"
      aria-label="Models matching @{partial?.alias}"
    >
      {#each suggestions as model (model.participant.id)}
        {@const alias = aliasesOf(model).find((a) =>
          a.startsWith(partial!.alias),
        )!}
        <button
          type="button"
          class="min-h-8 rounded border border-rule px-2 hover:bg-accent-soft"
          onclick={() => complete(alias)}
        >
          @{alias}
          <span class="text-muted">{model.participant.displayName}</span>
        </button>
      {/each}
    </div>
  {/if}

  <AskLine {answerers} bind:choice />
  {#if answerers.models.length > 0 && target}
    <button
      type="button"
      class="mt-1 rounded text-sm text-muted underline-offset-2 hover:text-ink hover:underline"
      onclick={previewFor}
    >
      What will be sent?
    </button>
  {/if}
</form>
