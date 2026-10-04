import type { Uuid } from "../types";

/**
 * Navigation state. On desktop both panes are visible and `screen` only
 * matters for focus; on phones it decides which single column is shown.
 */
export type Screen = "discussions" | "discussion";

export class Nav {
  screen: Screen = $state("discussions");
  selectedId: Uuid | null = $state(null);

  open(id: Uuid): void {
    this.selectedId = id;
    this.screen = "discussion";
  }

  back(): void {
    this.screen = "discussions";
  }
}

export const nav = new Nav();
