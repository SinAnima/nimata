// Attached files in the interface: labels, sizes, and which models can read
// them.

import type {
  AttachmentKind,
  Capabilities,
  ModelParticipant,
  ProviderView,
  StagedAttachment,
} from "./types";

/** As in nimata_core::attachments::MAX_ATTACHMENT_BYTES. */
export const MAX_ATTACHMENT_BYTES = 50 * 1024 * 1024;

export function humanSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} bytes`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

const TYPE_LABELS: Record<string, string> = {
  "text/markdown": "Markdown",
  "text/plain": "Text",
  "text/csv": "CSV",
  "application/json": "JSON",
  "application/pdf": "PDF",
  "image/png": "PNG image",
  "image/jpeg": "JPEG image",
  "image/gif": "GIF image",
  "image/webp": "WebP image",
};

/** A short name for a file's type, e.g. "Markdown" or "PDF". */
export function typeLabel(
  file: Pick<StagedAttachment, "mediaType" | "kind" | "filename">,
): string {
  const known = TYPE_LABELS[file.mediaType];
  if (known) return known;
  if (file.kind === "text") return "Text";
  const extension = file.filename.includes(".")
    ? file.filename.split(".").pop()!.toUpperCase()
    : "";
  return extension ? `${extension} file` : "File";
}

/** Whether a provider's models can read a kind of file. Text always goes. */
export function canRead(
  capabilities: Capabilities,
  kind: AttachmentKind,
): boolean {
  switch (kind) {
    case "text":
      return true;
    case "image":
      return capabilities.images;
    case "pdf":
      return capabilities.pdfs;
    case "other":
      return false;
  }
}

/** For each model, the files it cannot read. Only models with some. */
export function unreadable(
  models: readonly ModelParticipant[],
  providers: readonly ProviderView[],
  files: readonly StagedAttachment[],
): { model: ModelParticipant; files: StagedAttachment[] }[] {
  return models.flatMap((model) => {
    const provider = providers.find((p) => p.provider.id === model.providerId);
    if (!provider) return [];
    const missed = files.filter((f) => !canRead(provider.capabilities, f.kind));
    return missed.length > 0 ? [{ model, files: missed }] : [];
  });
}

/** Files from a drop, or none if the drag carries no files. */
export function droppedFiles(event: DragEvent): File[] {
  return [...(event.dataTransfer?.files ?? [])];
}

export function carriesFiles(event: DragEvent): boolean {
  return [...(event.dataTransfer?.types ?? [])].includes("Files");
}
