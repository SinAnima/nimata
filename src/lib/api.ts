// The only module that talks to the Rust core.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AppInfo,
  DiscussionSummary,
  DiscussionView,
  UnixMillis,
  Uuid,
} from "./types";

export const CLOCK_TICK = "nimata://clock-tick";

export function appInfo(): Promise<AppInfo> {
  return invoke("app_info");
}

export function listDiscussions(): Promise<DiscussionSummary[]> {
  return invoke("list_discussions");
}

export function getDiscussion(id: Uuid): Promise<DiscussionView> {
  return invoke("get_discussion", { id });
}

export function onClockTick(
  handler: (now: UnixMillis) => void,
): Promise<UnlistenFn> {
  return listen<UnixMillis>(CLOCK_TICK, (event) => handler(event.payload));
}
