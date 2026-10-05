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
}

export interface DiscussionView {
  discussion: Discussion;
  posts: Post[];
  participants: Participant[];
  draft: Draft | null;
}

export type DiscussionFilter = "active" | "archived";

export interface AppInfo {
  version: string;
  tauriVersion: string;
  databasePath: string | null;
  platform: string;
}
