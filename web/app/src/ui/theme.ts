// The theme: System, Light or Dark (DESIGN.md, Dark theme). The choice is a per-viewer
// convenience kept in `localStorage`, so it never reaches another device; the inline script in
// index.html applies it before first paint, and this module changes it afterwards and tells
// the screens (React Flow takes the resolved theme, so a forced one reaches the canvas too).
// Storage that refuses (a private window) leaves the choice in memory for the tab.
import { useSyncExternalStore } from "react";

export type ThemeChoice = "system" | "light" | "dark";
export type ResolvedTheme = "light" | "dark";

export const THEME_CHOICES: readonly ThemeChoice[] = ["system", "light", "dark"];

const KEY = "cairn:theme";
const DARK_QUERY = "(prefers-color-scheme: dark)";

/** The choice stored before this page, or System. */
export function storedTheme(): ThemeChoice {
  try {
    const stored = globalThis.localStorage.getItem(KEY);
    return stored === "light" || stored === "dark" ? stored : "system";
  } catch {
    return "system";
  }
}

let current: ThemeChoice = storedTheme();
const listeners = new Set<() => void>();

/** Applies `choice` to the page and keeps it for the next visit. */
export function setTheme(choice: ThemeChoice): void {
  current = choice;
  const root = globalThis.document.documentElement;
  if (choice === "system") {
    delete root.dataset["theme"];
  } else {
    root.dataset["theme"] = choice;
  }
  try {
    if (choice === "system") {
      globalThis.localStorage.removeItem(KEY);
    } else {
      globalThis.localStorage.setItem(KEY, choice);
    }
  } catch {
    // Kept for this tab only.
  }
  for (const listener of listeners) {
    listener();
  }
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  const query = globalThis.matchMedia(DARK_QUERY);
  query.addEventListener("change", listener);
  return () => {
    listeners.delete(listener);
    query.removeEventListener("change", listener);
  };
}

/** The theme the viewer chose, and a way to change it. */
export function useThemeChoice(): [ThemeChoice, (choice: ThemeChoice) => void] {
  const choice = useSyncExternalStore(subscribe, () => current);
  return [choice, setTheme];
}

/** The theme in effect: the choice, or the system's when the choice is System. */
export function resolveTheme(choice: ThemeChoice, systemDark: boolean): ResolvedTheme {
  return choice === "system" ? (systemDark ? "dark" : "light") : choice;
}

/** The theme in effect now, following the choice and the system's preference. */
export function useResolvedTheme(): ResolvedTheme {
  return useSyncExternalStore(
    subscribe,
    () => resolveTheme(current, globalThis.matchMedia(DARK_QUERY).matches),
    () => "light",
  );
}
