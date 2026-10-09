// The only module that talks to the Rust core.

import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AppInfo,
  AttachmentText,
  StagedAttachment,
  ArchiveExport,
  ImportReport,
  Generation,
  KeyStatus,
  ModelInfo,
  ModelParticipant,
  ProviderConfig,
  ProviderView,
  DatabaseStatus,
  Discussion,
  DiscussionFilter,
  DiscussionSummary,
  DiscussionView,
  Participant,
  Post,
  Revision,
  SearchResults,
  SentContext,
  UnixMillis,
  Uuid,
} from "./types";

export const CLOCK_TICK = "nimata://clock-tick";
export const POST_DELTA = "nimata://post-delta";
export const POST_UPDATED = "nimata://post-updated";

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
  attachments: StagedAttachment[] = [],
): Promise<DiscussionView> {
  return invoke("start_discussion", { title, body, attachments });
}

export function addPost(
  discussionId: Uuid,
  parentId: Uuid | null,
  body: string,
  contextIds: Uuid[] = [],
  attachments: StagedAttachment[] = [],
): Promise<Post> {
  return invoke("add_post", {
    discussionId,
    parentId,
    body,
    contextIds,
    attachments,
  });
}

/**
 * Stores a file for a post being written. The bytes go to Rust as the raw
 * request body, after the file's name and type (length-prefixed JSON).
 */
export async function stageAttachment(file: File): Promise<StagedAttachment> {
  const meta = new TextEncoder().encode(
    JSON.stringify({ filename: file.name, mediaType: file.type }),
  );
  const bytes = new Uint8Array(await file.arrayBuffer());
  const body = new Uint8Array(4 + meta.length + bytes.length);
  new DataView(body.buffer).setUint32(0, meta.length, true);
  body.set(meta, 4);
  body.set(bytes, 4 + meta.length);
  return invoke("stage_attachment", body);
}

/** Forgets a file removed before posting, unless something else uses it. */
export function discardStaged(contentHash: string): Promise<void> {
  return invoke("discard_staged", { contentHash });
}

export function attachmentText(contentHash: string): Promise<AttachmentText> {
  return invoke("attachment_text", { contentHash });
}

/** Asks where to save, then writes a copy of an attached file there. */
export function saveAttachment(id: Uuid): Promise<string | null> {
  return invoke("save_attachment", { id });
}

/** The address an attached image is shown from. */
export function attachmentUrl(contentHash: string): string {
  return convertFileSrc(contentHash.replace(/^sha256:/, ""), "attachment");
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
  contextIds: Uuid[] = [],
  attachments: StagedAttachment[] = [],
): Promise<void> {
  return invoke("save_draft", {
    discussionId,
    parentId,
    body,
    contextIds,
    attachments,
  });
}

/**
 * Exactly what a model would be sent. With `draft`, as if the draft were
 * posted (replying to `parentId`, with `contextIds`) and the model then
 * asked; without, as if asked to reply to `parentId` now.
 */
export function previewContext(
  discussionId: Uuid,
  parentId: Uuid,
  participantId: Uuid,
  draft: string | null,
  contextIds: Uuid[] = [],
  attachments: StagedAttachment[] = [],
): Promise<SentContext> {
  return invoke("preview_context", {
    discussionId,
    parentId,
    participantId,
    draft,
    contextIds,
    attachments,
  });
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
/** Asks where to save, then writes the discussion as Markdown. */
export function exportMarkdown(id: Uuid): Promise<string | null> {
  return invoke("export_markdown", { id });
}

/** Asks where to save, then writes every discussion into one archive. */
export function exportArchive(): Promise<ArchiveExport | null> {
  return invoke("export_archive");
}

/** Asks for a Nimata or ChatGPT export, then imports it. */
export function importFile(): Promise<ImportReport | null> {
  return invoke("import_file");
}

export function backupDatabase(): Promise<string | null> {
  return invoke("backup_database");
}

/** Asks for a backup file; resolves to its path, or null if cancelled. */
export function restoreDatabase(): Promise<string | null> {
  return invoke("restore_database");
}

export function providers(): Promise<ProviderView[]> {
  return invoke("providers");
}

export function saveApiKey(providerId: Uuid, key: string): Promise<KeyStatus> {
  return invoke("save_api_key", { providerId, key });
}

export function removeApiKey(providerId: Uuid): Promise<KeyStatus> {
  return invoke("remove_api_key", { providerId });
}

/** Lists the provider's text models; also proves the key works. */
export function providerModels(providerId: Uuid): Promise<ModelInfo[]> {
  return invoke("provider_models", { providerId });
}

export function setModel(
  providerId: Uuid,
  model: string,
  displayName: string,
  enabled: boolean,
): Promise<ModelParticipant> {
  return invoke("set_model", { providerId, model, displayName, enabled });
}

/** Starts a model reply; it streams in through post events. */
export function askModel(
  discussionId: Uuid,
  parentId: Uuid,
  participantId: Uuid,
): Promise<Post> {
  return invoke("ask_model", { discussionId, parentId, participantId });
}

export function cancelReply(postId: Uuid): Promise<boolean> {
  return invoke("cancel_reply", { postId });
}

export function retryReply(postId: Uuid): Promise<Post> {
  return invoke("retry_reply", { postId });
}

export function replyDetails(postId: Uuid): Promise<Generation | null> {
  return invoke("reply_details", { postId });
}

/** Adds an OpenAI-compatible connection, e.g. a local Ollama server. */
export function addEndpoint(
  displayName: string,
  baseUrl: string,
  key: string | null,
): Promise<ProviderConfig> {
  return invoke("add_endpoint", { displayName, baseUrl, key });
}

export function updateEndpoint(
  providerId: Uuid,
  displayName: string,
  baseUrl: string,
): Promise<ProviderConfig> {
  return invoke("update_endpoint", { providerId, displayName, baseUrl });
}

export function removeEndpoint(providerId: Uuid): Promise<void> {
  return invoke("remove_endpoint", { providerId });
}

export function setModelAliases(
  participantId: Uuid,
  aliases: string[],
): Promise<ModelParticipant> {
  return invoke("set_model_aliases", { participantId, aliases });
}

export function defaultModel(): Promise<Uuid | null> {
  return invoke("default_model");
}

export function setDefaultModel(participantId: Uuid | null): Promise<void> {
  return invoke("set_default_model", { participantId });
}

export function search(query: string): Promise<SearchResults> {
  return invoke("search", { query });
}

/** Remembers a search that was used; returns the recent searches. */
export function recordSearch(query: string): Promise<string[]> {
  return invoke("record_search", { query });
}

export function recentSearches(): Promise<string[]> {
  return invoke("recent_searches");
}

export function clearRecentSearches(): Promise<void> {
  return invoke("clear_recent_searches");
}

export function onPostDelta(
  handler: (delta: { postId: Uuid; text: string }) => void,
): Promise<UnlistenFn> {
  return listen<{ postId: Uuid; text: string }>(POST_DELTA, (e) =>
    handler(e.payload),
  );
}

export function onPostUpdated(
  handler: (post: Post) => void,
): Promise<UnlistenFn> {
  return listen<Post>(POST_UPDATED, (e) => handler(e.payload));
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
