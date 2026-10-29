// Keys never fire while a field has focus: the one test every keyboard binding asks first.

/** Whether a key press belongs to a field being typed in, not to the page. */
export function typing(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && (target.isContentEditable || ["INPUT", "SELECT", "TEXTAREA"].includes(target.tagName));
}

/** Whether a popover or dialog is open: Esc closes it first, and nothing behind it hears that press. */
export function overlayOpen(): boolean {
  return document.querySelector("[data-popover], dialog[open]") !== null;
}
