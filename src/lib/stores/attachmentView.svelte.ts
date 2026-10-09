import type { AttachmentText, StagedAttachment, Uuid } from "../types";
import { attachmentText, errorMessage } from "../api";

/** A file being previewed: posted (with an ID) or still waiting to post. */
export type PreviewFile = StagedAttachment & { id?: Uuid };

/** The attachment preview: an image, or the beginning of a text file. */
export const attachmentView: {
  file: PreviewFile | null;
  text: AttachmentText | null;
  error: string | null;
} = $state({ file: null, text: null, error: null });

export async function previewAttachment(file: PreviewFile): Promise<void> {
  attachmentView.file = file;
  attachmentView.text = null;
  attachmentView.error = null;
  if (file.kind !== "text") return;
  try {
    const text = await attachmentText(file.contentHash);
    if (attachmentView.file === file) attachmentView.text = text;
  } catch (e) {
    if (attachmentView.file === file) attachmentView.error = errorMessage(e);
  }
}

export function closePreview(): void {
  attachmentView.file = null;
}

/** Files that can be shown in Nimata; others can only be saved. */
export function previewable(file: StagedAttachment): boolean {
  return file.kind === "text" || file.kind === "image";
}
