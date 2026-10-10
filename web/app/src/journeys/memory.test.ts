// The last projection is remembered per journey, and the screens work without the
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
  it("is each journey's own, and the graph until one is remembered", () => {
    vi.stubGlobal("localStorage", storage());
    expect(recalledProjection("j_a")).toBe("graph");
    rememberProjection("j_a", "timeline");
    expect(recalledProjection("j_a")).toBe("timeline");
    expect(recalledProjection("j_b")).toBe("graph");
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
    expect(() => { rememberProjection("j_a", "list"); }).not.toThrow();
    expect(recalledProjection("j_a")).toBe("graph");
  });
});
