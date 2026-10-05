// Keyboard helpers shared by the shortcut handler and the forms.

const isMac =
  typeof navigator !== "undefined" &&
  /Mac|iPhone|iPad/.test(navigator.platform);

/** The platform's command modifier: Cmd on Apple platforms, Ctrl elsewhere. */
export function hasModifier(event: KeyboardEvent): boolean {
  return isMac ? event.metaKey : event.ctrlKey;
}

export function isSubmitShortcut(event: KeyboardEvent): boolean {
  return event.key === "Enter" && hasModifier(event);
}

/** Single-letter shortcuts must not fire while the user is typing. */
export function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return (
    target.isContentEditable ||
    ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)
  );
}

export const MODIFIER_LABEL = isMac ? "⌘" : "Ctrl+";

/**
 * The post to move to with J (next) or K (previous). With nothing focused,
 * J starts at the first post and K at the last.
 */
export function adjacentPost(
  ids: readonly string[],
  current: string | null,
  direction: 1 | -1,
): string | undefined {
  const index = current === null ? -1 : ids.indexOf(current);
  if (index === -1) return direction === 1 ? ids[0] : ids[ids.length - 1];
  return ids[Math.min(Math.max(index + direction, 0), ids.length - 1)];
}
