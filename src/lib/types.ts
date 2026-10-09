// Mirrors the serde types in crates/nimata-core/src/domain.rs.

export type Uuid = string;
/** UTC instant in Unix milliseconds. */
export type UnixMillis = number;

export type ParticipantKind = "human" | "model" | "agent";

export interface Participant {
  id: Uuid;
  kind: ParticipantKind;
  displayName: string;
  provider: string | null;
  model: string | null;
}

export interface Discussion {
  id: Uuid;
  title: string;
  createdAt: UnixMillis;
  updatedAt: UnixMillis;
  archivedAt: UnixMillis | null;
  pinnedAt: UnixMillis | null;
}

export type PostStatus = "complete" | "streaming" | "failed" | "cancelled";

export interface Post {
  id: Uuid;
  discussionId: Uuid;
  parentId: Uuid | null;
  authorId: Uuid;
  body: string;
  createdAt: UnixMillis;
  /** Author's UTC offset in minutes when the post was written. */
  tzOffsetMinutes: number;
  editedAt: UnixMillis | null;
  /** Set when deleted; the post stays as a tombstone with an empty body. */
  deletedAt: UnixMillis | null;
  status: PostStatus;
  /** For model replies: which provider and model version answered. */
  providerMetadata: ProviderMetadata | null;
  /** Other posts chosen as context, besides the one replied to. */
  contextIds: Uuid[];
}

export interface ProviderMetadata {
  provider: string;
  model: string | null;
  responseId: string | null;
  requestId: string | null;
  inputTokens: number | null;
  outputTokens: number | null;
  incompleteReason: string | null;
}

export type ProviderKind = "openai" | "anthropic" | "openai_compatible";

export interface ProviderConfig {
  id: Uuid;
  kind: ProviderKind;
  displayName: string;
  baseUrl: string | null;
}

export interface KeyStatus {
  /** Where the key in use comes from, or null when there is none. */
  source: "saved" | "environment" | null;
  /** Last four characters, in development builds only. */
  hint: string | null;
  /** Where saved keys are kept, e.g. "the Keychain". */
  store: string;
  /** The environment variable read in development builds, if any. */
  environmentVariable: string | null;
}

export interface ModelParticipant {
  participant: Participant;
  providerId: Uuid;
  enabled: boolean;
  /** Names it can be mentioned by, without "@". */
  aliases: string[];
}

/** What a kind of provider can do. */
export interface Capabilities {
  requiresKey: boolean;
  customEndpoint: boolean;
  multiple: boolean;
  modelDiscovery: boolean;
  streaming: boolean;
  usage: boolean;
  /** Images can be sent to its models. */
  images: boolean;
  /** PDF documents can be sent to its models. */
  pdfs: boolean;
}

export interface ProviderView {
  provider: ProviderConfig;
  capabilities: Capabilities;
  key: KeyStatus;
  models: ModelParticipant[];
}

export interface ModelInfo {
  id: string;
}

export type GenerationStatus =
  "queued" | "sending" | "streaming" | "complete" | "failed" | "cancelled";

export interface Generation {
  id: Uuid;
  postId: Uuid;
  participantId: Uuid;
  status: GenerationStatus;
  error: string | null;
  /** Exactly which posts were sent to the model, in order. */
  contextPostIds: Uuid[];
  startedAt: UnixMillis;
  finishedAt: UnixMillis | null;
  /** Exactly what was sent; recorded from schema version 5. */
  sent: SentContext | null;
}

export type ContextSource = "thread" | "context";

export interface SentMessage {
  role: "user" | "assistant";
  text: string;
  source: ContextSource;
  postIds: Uuid[];
  /** Files sent with the message; text files are inside `text`. */
  attachments?: SentAttachment[];
}

export interface SentAttachment {
  id: Uuid;
  filename: string;
  mediaType: string;
  size: number;
  contentHash: string;
  delivery: "text" | "image" | "pdf";
}

export interface OmittedAttachment {
  id: Uuid;
  postId: Uuid;
  filename: string;
  reason: "unsupported" | "too_large" | "missing";
}

export interface Omitted {
  postId: Uuid;
  source: ContextSource;
  reason: "deleted" | "unfinished" | "trimmed";
}

/** Exactly what a model is, or was, sent. */
export interface SentContext {
  model: string;
  instructions: string;
  messages: SentMessage[];
  omitted: Omitted[];
  estimatedTokens: number;
  budgetTokens: number;
  /** Files on sent posts that this model could not take. */
  omittedAttachments?: OmittedAttachment[];
}

/** An earlier version of an edited post. */
export interface Revision {
  postId: Uuid;
  body: string;
  /** When this text became the post's body. */
  writtenAt: UnixMillis;
  /** When an edit replaced it. */
  replacedAt: UnixMillis;
}

export interface DatabaseStatus {
  path: string | null;
  /** Why the database could not be opened, if it could not. */
  error: string | null;
}

export interface DiscussionSummary {
  id: Uuid;
  title: string;
  lastActivityAt: UnixMillis;
  postCount: number;
  excerpt: string;
}

export interface Draft {
  discussionId: Uuid;
  parentId: Uuid | null;
  body: string;
  updatedAt: UnixMillis;
  /** Posts chosen as context for the unsent post. */
  contextIds: Uuid[];
  /** Files added to the unsent post. */
  attachments?: StagedAttachment[];
}

export interface DiscussionView {
  discussion: Discussion;
  posts: Post[];
  participants: Participant[];
  draft: Draft | null;
  /** Files attached to the posts, in the order attached. */
  attachments: Attachment[];
}

export type DiscussionFilter = "active" | "archived";

export interface AppInfo {
  version: string;
  tauriVersion: string;
  databasePath: string | null;
  platform: string;
}

/** A post that matched a search. */
export interface PostHit {
  postId: Uuid;
  discussionId: Uuid;
  discussionTitle: string;
  authorName: string;
  createdAt: UnixMillis;
  /** Text around the match; matches are between U+E000 and U+E001. */
  snippet: string;
}

/** A discussion whose title matched a search. */
export interface DiscussionHit {
  discussionId: Uuid;
  /** The title, with matches marked like snippets. */
  title: string;
  lastActivityAt: UnixMillis;
  archived: boolean;
}

export interface SearchResults {
  posts: PostHit[];
  /** More posts matched than were returned. */
  morePosts: boolean;
  discussions: DiscussionHit[];
  /** Parts of the query that could not be used, explained. */
  warnings: string[];
  /** Words and phrases to highlight in a discussion opened from a result. */
  highlight: string[];
}

export type ImportSource = "nimataDiscussion" | "nimataArchive" | "chatGpt";

export interface ImportOutcome {
  discussionId: Uuid;
  title: string;
  result: "added" | "updated" | "unchanged" | "skippedDeleted";
  postsAdded: number;
  postsPresent: number;
  attachmentsAdded: number;
  /** Files listed on imported posts but not in the file imported. */
  attachmentsMissing: number;
}

export interface ImportReport {
  source: ImportSource;
  outcomes: ImportOutcome[];
  /** Discussions that could not be imported, with the reason. */
  failures: string[];
}

export interface ArchiveExport {
  path: string;
  discussions: number;
}

export type AttachmentKind = "text" | "image" | "pdf" | "other";

/** A file stored and waiting to be posted. */
export interface StagedAttachment {
  filename: string;
  mediaType: string;
  size: number;
  /** `sha256:<hex>` of the bytes. */
  contentHash: string;
  kind: AttachmentKind;
}

/** A file attached to a post. */
export interface Attachment extends StagedAttachment {
  id: Uuid;
  postId: Uuid;
  createdAt: UnixMillis;
}

export interface AttachmentText {
  text: string;
  /** Only the beginning is included. */
  truncated: boolean;
}
