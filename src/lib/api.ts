// The only module that talks to the Rust core.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AppInfo,
  DatabaseStatus,
  Discussion,
  DiscussionFilter,
  DiscussionSummary,
  DiscussionView,
  Participant,
  Post,
  Revision,
  UnixMillis,
  Uuid,
} from "./types";

export const CLOCK_TICK = "nimata://clock-tick";

export function appInfo(): Promise<AppInfo> {
  return invoke("app_info");
}

export function localUser(): Promise<Participant> {
  return invoke("local_user");
}

export function renameLocalUser(name: string): Promise<Participant> {
  return invoke("rename_local_user", { name });
}

export function listDiscussions(
  filter: DiscussionFilter,
): Promise<DiscussionSummary[]> {
  return invoke("list_discussions", { filter });
}

export function getDiscussion(id: Uuid): Promise<DiscussionView> {
  return invoke("get_discussion", { id });
}

export function startDiscussion(
  title: string,
  body: string,
): Promise<DiscussionView> {
  return invoke("start_discussion", { title, body });
}

export function addPost(
  discussionId: Uuid,
  parentId: Uuid | null,
  body: string,
): Promise<Post> {
  return invoke("add_post", { discussionId, parentId, body });
}

export function renameDiscussion(id: Uuid, title: string): Promise<Discussion> {
  return invoke("rename_discussion", { id, title });
}

export function setArchived(id: Uuid, archived: boolean): Promise<Discussion> {
  return invoke("set_archived", { id, archived });
}

export function saveDraft(
  discussionId: Uuid,
  parentId: Uuid | null,
  body: string,
): Promise<void> {
  return invoke("save_draft", { discussionId, parentId, body });
}

export function editPost(postId: Uuid, body: string): Promise<Post> {
  return invoke("edit_post", { postId, body });
}

export function postRevisions(postId: Uuid): Promise<Revision[]> {
  return invoke("post_revisions", { postId });
}

export function deletePost(postId: Uuid): Promise<Post> {
  return invoke("delete_post", { postId });
}

export function deleteDiscussion(id: Uuid): Promise<void> {
  return invoke("delete_discussion", { id });
}

export function databaseStatus(): Promise<DatabaseStatus> {
  return invoke("database_status");
}

/** Asks where to save; resolves to the saved path, or null if cancelled. */
export function exportDiscussion(id: Uuid): Promise<string | null> {
  return invoke("export_discussion", { id });
}

/** Asks where to save; resolves to the saved path, or null if cancelled. */
export function backupDatabase(): Promise<string | null> {
  return invoke("backup_database");
}

/** Asks for a backup file; resolves to its path, or null if cancelled. */
export function restoreDatabase(): Promise<string | null> {
  return invoke("restore_database");
}

export function onClockTick(
  handler: (now: UnixMillis) => void,
): Promise<UnlistenFn> {
  return listen<UnixMillis>(CLOCK_TICK, (event) => handler(event.payload));
}

/** Tauri rejects with the Rust error string; anything else is unexpected. */
export function errorMessage(error: unknown): string {
  return typeof error === "string"
    ? error
    : error instanceof Error
      ? error.message
      : "Something went wrong.";
}
