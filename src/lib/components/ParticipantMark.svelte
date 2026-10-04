<script lang="ts">
  import type { ParticipantKind } from "../types";

  let { kind }: { kind: ParticipantKind } = $props();
</script>

<!-- Shape, not color, distinguishes people (filled) from models (ring). -->
<span
  aria-hidden="true"
  class={[
    "inline-block size-2 shrink-0 translate-y-[-1px] rounded-full border-[1.5px]",
    kind === "human" ? "border-ink bg-ink" : "border-accent bg-transparent",
  ]}
></span>
<span class="sr-only">{kind === "human" ? "Person" : "Model"}:</span>
