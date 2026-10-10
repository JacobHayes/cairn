// ARCHITECTURE, Web UI: drafts survive a reload. Text being typed and forms not yet sent are
// kept per tab in session storage, so a reload, a crash, or a version-skew reload loses
// nothing; anything sent is already committed on the host. Storage that refuses (a private
// window, a full quota) leaves the draft in memory only.
import { useCallback, useState } from "react";

const PREFIX = "cairn:draft:";

function storage(): Storage | undefined {
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
    const text = storage()?.getItem(PREFIX + key);
    return text == null ? undefined : (JSON.parse(text) as T);
  } catch {
    return undefined;
  }
}

/** Every draft whose key starts with `prefix`, by key. */
export function readDrafts<T>(prefix: string): [string, T][] {
  const found: [string, T][] = [];
  try {
    const held = storage();
    for (let at = 0; held !== undefined && at < held.length; at += 1) {
      const key = held.key(at);
      if (key?.startsWith(PREFIX + prefix) === true) {
        found.push([key.slice(PREFIX.length), JSON.parse(held.getItem(key) ?? "null") as T]);
      }
    }
  } catch {
    // None are kept.
  }
  return found;
}

/** Keeps `value` under `key`, or forgets the draft when it is undefined (sent or abandoned). */
export function writeDraft(key: string, value: unknown): void {
  try {
    if (value === undefined) {
      storage()?.removeItem(PREFIX + key);
    } else {
      storage()?.setItem(PREFIX + key, JSON.stringify(value));
    }
  } catch {
    // Kept in memory only.
  }
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
