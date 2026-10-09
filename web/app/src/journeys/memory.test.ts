// The last projection per page is remembered per journey, and the screens work without the
// store: localStorage can be missing or throw (a private window, blocked site data).
import { afterEach, describe, expect, it, vi } from "vitest";

import { recalledProjection, rememberProjection } from "./memory.ts";

function storage(): Storage {
  const held = new Map<string, string>();
  return {
    getItem: (key) => held.get(key) ?? null,
    setItem: (key, value) => void held.set(key, value),
  } as Storage;
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("the remembered projection", () => {
  it("is each page's own per journey, and the page's default until one is remembered", () => {
    vi.stubGlobal("localStorage", storage());
    expect(recalledProjection("j_a", "plan")).toBe("graph");
    rememberProjection("j_a", "plan", "timeline");
    rememberProjection("j_a", "next", "cards");
    expect(recalledProjection("j_a", "plan")).toBe("timeline");
    expect(recalledProjection("j_a", "next")).toBe("cards");
    expect(recalledProjection("j_b", "plan")).toBe("graph");
  });

  it("falls back to the default when the store throws", () => {
    const refusing = {
      getItem: () => {
        throw new Error("blocked");
      },
      setItem: () => {
        throw new Error("blocked");
      },
    } as unknown as Storage;
    vi.stubGlobal("localStorage", refusing);
    expect(() => { rememberProjection("j_a", "plan", "list"); }).not.toThrow();
    expect(recalledProjection("j_a", "plan")).toBe("graph");
  });
});
