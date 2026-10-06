import type { SentContext } from "../types";
import { errorMessage } from "../api";

/** The context dialog: what a model will be, or was, sent. */
export const contextView: {
  open: boolean;
  title: string;
  sent: SentContext | null;
  error: string | null;
  loading: boolean;
} = $state({ open: false, title: "", sent: null, error: null, loading: false });

export async function showContext(
  title: string,
  load: () => Promise<SentContext | null>,
): Promise<void> {
  contextView.title = title;
  contextView.sent = null;
  contextView.error = null;
  contextView.loading = true;
  contextView.open = true;
  try {
    contextView.sent = await load();
    if (!contextView.sent)
      contextView.error = "No record was kept of what this reply was sent.";
  } catch (e) {
    contextView.error = errorMessage(e);
  } finally {
    contextView.loading = false;
  }
}

export function closeContext(): void {
  contextView.open = false;
}
