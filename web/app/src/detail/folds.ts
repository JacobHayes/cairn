// C8: which of the inspector's sections are folded, remembered per viewer (design 6.8) in
// localStorage, keyed by the kind of node and the section. It can be missing or throw (a private
// window, blocked site data, a preview), so every read and write is guarded and the sections
// then take their defaults. A section can be opened from outside it (the `⋯` menu's "Pin a
// date…"), so the state is a small store the sections subscribe to.
import { useSyncExternalStore } from "react";

const PREFIX = "cairn:fold:";

/** What this tab holds, over what storage keeps, so a section opened by a menu stays open without storage. */
const held = new Map<string, boolean>();
const listeners = new Set<() => void>();

function read(key: string): boolean | undefined {
  if (held.has(key)) {
    return held.get(key);
  }
  try {
    const stored = globalThis.localStorage.getItem(`${PREFIX}${key}`);
    return stored === null ? undefined : stored === "open";
  } catch {
    return undefined;
  }
}

/** Remembers that section `key` is open or folded, and tells the sections showing it. */
export function setFold(key: string, open: boolean): void {
  held.set(key, open);
  try {
    globalThis.localStorage.setItem(`${PREFIX}${key}`, open ? "open" : "folded");
  } catch {
    // Not remembered beyond this tab's life.
  }
  listeners.forEach((listen) => {
    listen();
  });
}

/** The fold key of a section of a node of `kind`. */
export function foldKey(kind: string, section: string): string {
  return `${kind}:${section}`;
}

/** Whether the section is open: what the viewer last chose, else `fallback`. */
export function useFold(key: string | undefined, fallback: boolean): boolean {
  const stored = useSyncExternalStore(
    (listen) => {
      listeners.add(listen);
      return () => {
        listeners.delete(listen);
      };
    },
    () => (key === undefined ? undefined : read(key)),
    () => undefined,
  );
  return stored ?? fallback;
}
