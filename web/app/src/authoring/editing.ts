// A role, a kind or a resource being edited is a draft this tab keeps (data/drafts.ts): the value
// as typed, the revision its author opened it at (H5), and the value as it opened, so what is
// unsent is what differs from it. Publish waits for it (unsaved.ts).

/** A value being edited, kept across reloads. */
export interface Editing<T> {
  value: T;
  base: number;
  /** The value as it opened. */
  opened: T;
}

/** Whether `editing` holds input that Publish would leave out: its value differs from how it opened. */
export function unsent<T>(editing: Editing<T>): boolean {
  return JSON.stringify(editing.value) !== JSON.stringify(editing.opened);
}

