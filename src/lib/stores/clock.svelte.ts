import { onClockTick } from "../api";

/** Shared "now" for relative timestamps, advanced by the Rust clock event. */
export const clock = $state({ now: Date.now() });

export function startClock(): Promise<() => void> {
  return onClockTick((now) => {
    clock.now = now;
  });
}
