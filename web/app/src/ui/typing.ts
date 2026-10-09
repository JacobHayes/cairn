// Keys never fire while a field has focus (3.4): the one test every keyboard binding asks first.

/** Whether a key press belongs to a field being typed in, not to the page. */
export function typing(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && (target.isContentEditable || ["INPUT", "SELECT", "TEXTAREA"].includes(target.tagName));
}
