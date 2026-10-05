<script lang="ts">
  import type { Answerers, AskChoice } from "../compose";
  import { describeProblem } from "../mentions";
  import { notebook } from "../stores/notebook.svelte";

  let {
    answerers,
    choice = $bindable(),
  }: { answerers: Answerers; choice: AskChoice } = $props();

  const reasonText: Record<string, string> = {
    replying: "you are replying to it",
    "last in thread": "last to reply here",
    default: "default",
  };
</script>

{#if notebook.askableModels.length > 0}
  <div class="mt-1.5 text-sm text-muted">
    {#if answerers.mentions.problems.length > 0}
      {#each answerers.mentions.problems as problem (problem.alias)}
        <p role="alert">{describeProblem(problem)}</p>
      {/each}
    {:else if answerers.reason === "mentioned"}
      <p>
        Then asks {answerers.models
          .map((m) => m.participant.displayName)
          .join(", ")}
      </p>
    {:else}
      <label class="flex items-center gap-2">
        <span>Then ask</span>
        <select
          class="rounded border border-rule bg-transparent px-2 py-1 text-ink"
          value={choice ?? "auto"}
          onchange={(e) => {
            const value = e.currentTarget.value;
            choice = value === "auto" ? null : value;
          }}
        >
          <option value="auto">
            {#if choice === null && answerers.models[0]}
              {answerers.models[0].participant.displayName} ({reasonText[
                answerers.reason
              ]})
            {:else if choice === null}
              No one (no model is set as default)
            {:else}
              Decide automatically
            {/if}
          </option>
          {#each notebook.askableModels as model (model.participant.id)}
            <option value={model.participant.id}>
              {model.participant.displayName}
            </option>
          {/each}
          <option value="none">No one</option>
        </select>
      </label>
    {/if}
  </div>
{/if}
