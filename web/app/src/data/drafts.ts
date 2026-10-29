// ARCHITECTURE, Web UI: drafts survive a reload. Text being typed and forms not yet sent are
// kept per tab in session storage, so a reload, a crash, or a version-skew reload loses
// nothing. An edit already sent is no longer a draft: the edit manager keeps its record
// (edit-manager.ts). Storage that refuses (a private window, a full quota) leaves the draft in
// memory only.
import { useCallback, useState, useSyncExternalStore } from "react";

/** What every draft's storage key starts with. */
export const DRAFT_PREFIX = "cairn:draft:";
const PREFIX = DRAFT_PREFIX;

/** This tab's session storage, or none where the browser refuses it. */
export function tabStorage(): Storage | undefined {
  try {
    return globalThis.sessionStorage;
  } catch {
    return undefined;
  }
}

/** The draft kept under `key`, if any. */
// eslint-disable-next-line @typescript-eslint/no-unnecessary-type-parameters -- the caller's draft shape fixes T
export function readDraft<T>(key: string): T | undefined {
  try {
    const text = tabStorage()?.getItem(PREFIX + key);
    return text == null ? (memoryOnly.get(key) as T | undefined) : (JSON.parse(text) as T);
  } catch {
    return memoryOnly.get(key) as T | undefined;
  }
}

const watchers = new Set<() => void>();
/** The drafts storage refused: kept here for this page's life, so they still come back and still count as unsent. */
const memoryOnly = new Map<string, unknown>();

/** Keeps `value` under `key`, or forgets the draft when it is undefined (sent or abandoned). */
export function writeDraft(key: string, value: unknown): void {
  memoryOnly.delete(key);
  try {
    if (value === undefined) {
      tabStorage()?.removeItem(PREFIX + key);
    } else {
      tabStorage()?.setItem(PREFIX + key, JSON.stringify(value));
    }
  } catch {
    if (value !== undefined) {
      memoryOnly.set(key, value);
    }
  }
  for (const watcher of watchers) {
    watcher();
  }
}

/** The keys, after `prefix`, of the drafts this tab keeps whose key starts with it. */
export function draftKeys(prefix: string): string[] {
  const storage = tabStorage();
  const kept = new Set([...memoryOnly.keys()].filter((key) => key.startsWith(prefix)).map((key) => key.slice(prefix.length)));
  try {
    for (let at = 0; at < (storage?.length ?? 0); at += 1) {
      const key = storage?.key(at);
      if (key?.startsWith(PREFIX + prefix) === true) {
        kept.add(key.slice(PREFIX.length + prefix.length));
      }
    }
  } catch {
    // Only what this tab wrote.
  }
  return [...kept].sort();
}

/** Calls `notify` whenever a draft is kept or forgotten; stops when the returned function is called. */
function watch(notify: () => void): () => void {
  watchers.add(notify);
  return () => watchers.delete(notify);
}

/** The keys, after `prefix`, of the drafts kept under it, read again whenever a draft is kept or forgotten. */
export function useDraftKeys(prefix: string): string[] {
  const joined = useSyncExternalStore(watch, () => draftKeys(prefix).join("\n"));
  return joined === "" ? [] : joined.split("\n");
}

/**
 * The keys, after `prefix`, of the drafts kept under it that `holds` something worth keeping, read
 * again whenever any draft is written (a draft's contents change while its key stays).
 */
// eslint-disable-next-line @typescript-eslint/no-unnecessary-type-parameters -- the caller's draft shape fixes T
export function useDraftKeysHolding<T>(prefix: string, holds: (draft: T) => boolean): string[] {
  // A JSON list, so a key that is empty is still one.
  const kept = useSyncExternalStore(watch, () => JSON.stringify(draftKeys(prefix).filter((key) => holdsAt(prefix + key, holds))));
  return JSON.parse(kept) as string[];
}

/** Whether the draft kept under exactly `key` holds something worth keeping (a single draft, not a family sharing a prefix). */
// eslint-disable-next-line @typescript-eslint/no-unnecessary-type-parameters -- the caller's draft shape fixes T
export function useDraftHolding<T>(key: string, holds: (draft: T) => boolean): boolean {
  return useSyncExternalStore(watch, () => holdsAt(key, holds));
}

// eslint-disable-next-line @typescript-eslint/no-unnecessary-type-parameters -- the caller's draft shape fixes T
function holdsAt<T>(key: string, holds: (draft: T) => boolean): boolean {
  const draft = readDraft<T>(key);
  return draft !== undefined && holds(draft);
}

/** A draft held with the key it was read under. */
interface Held<T> {
  key: string;
  value: T | undefined;
}

/** `held` while it is `key`'s draft; otherwise what is kept under `key`, so a draft never follows a screen to another journey or node. */
export function draftAt<T>(held: Held<T>, key: string): Held<T> {
  return held.key === key ? held : { key, value: readDraft<T>(key) };
}

/**
 * A draft kept under `key` across reloads of this tab: the value and its setter. When `key`
 * changes (a screen reused for another journey), the value is the new key's draft.
 */
export function useDraft<T>(key: string): [T | undefined, (value: T | undefined) => void] {
  const [state, setState] = useState<Held<T>>(() => ({ key, value: readDraft<T>(key) }));
  const current = draftAt(state, key);
  if (current !== state) {
    setState(current);
  }
  const set = useCallback(
    (next: T | undefined) => {
      writeDraft(key, next);
      setState({ key, value: next });
    },
    [key],
  );
  return [current.value, set];
}
