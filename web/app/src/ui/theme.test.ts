// The theme the viewer chose is applied to the page and kept for the next visit; System clears both.
import { afterEach, expect, it, vi } from "vitest";

import { setTheme, storedTheme } from "./theme.ts";

afterEach(() => {
  vi.unstubAllGlobals();
});

it("keeps a chosen theme for the next visit and drops it for System", () => {
  const held = new Map<string, string>();
  const dataset: Record<string, string> = {};
  vi.stubGlobal("localStorage", {
    getItem: (key: string) => held.get(key) ?? null,
    setItem: (key: string, value: string) => void held.set(key, value),
    removeItem: (key: string) => void held.delete(key),
  });
  vi.stubGlobal("document", { documentElement: { dataset } });
  setTheme("dark");
  expect([dataset["theme"], storedTheme()]).toEqual(["dark", "dark"]);
  setTheme("system");
  expect([dataset["theme"], storedTheme()]).toEqual([undefined, "system"]);
});
