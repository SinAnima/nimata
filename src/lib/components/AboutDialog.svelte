<script lang="ts">
  import { appInfo } from "../api";
  import type { AppInfo } from "../types";

  let dialog: HTMLDialogElement | undefined = $state();
  let info: AppInfo | null = $state(null);

  export async function open(): Promise<void> {
    dialog?.showModal();
    info ??= await appInfo().catch(() => null);
  }
</script>

<dialog
  bind:this={dialog}
  aria-labelledby="about-title"
  class="m-auto w-[min(32rem,calc(100vw-2rem))] rounded-lg border border-rule bg-surface p-0 text-ink shadow-xl backdrop:bg-black/30"
>
  <div class="p-6">
    <h2 id="about-title" class="font-serif text-2xl">Nimata</h2>
    <p class="mt-2 font-serif leading-relaxed">
      A local-first discussion client for conversations with people and AI
      models. Your discussions are stored on this device and belong to you.
    </p>

    <dl class="mt-5 grid grid-cols-[auto_1fr] gap-x-6 gap-y-1.5 text-sm">
      <dt class="text-muted">Version</dt>
      <dd class="tabular-nums">{info?.version ?? "Unavailable"}</dd>
      <dt class="text-muted">Tauri</dt>
      <dd class="tabular-nums">{info?.tauriVersion ?? "Unavailable"}</dd>
      <dt class="text-muted">Platform</dt>
      <dd>{info?.platform ?? "Unavailable"}</dd>
      <dt class="text-muted">Data folder</dt>
      <dd class="break-all">{info?.dataDir ?? "Unavailable"}</dd>
      <dt class="text-muted">Appearance</dt>
      <dd>Follows your system light or dark setting</dd>
      <dt class="text-muted">Privacy</dt>
      <dd>No account, no telemetry</dd>
    </dl>
  </div>
  <form method="dialog" class="flex justify-end border-t border-rule px-6 py-3">
    <button
      class="min-h-10 rounded px-4 text-sm font-medium text-accent hover:bg-accent-soft"
    >
      Close
    </button>
  </form>
</dialog>
