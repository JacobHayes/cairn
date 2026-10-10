// ARCHITECTURE, Web UI: drafts survive a reload. What is typed is kept per tab under a key naming
// its journey and node, so it comes back after a reload, goes when sent, and never follows a
// screen reused for another journey or node.
import { afterEach, describe, expect, it, vi } from "vitest";

import { draftAt, readDraft, readDrafts, writeDraft } from "./drafts.ts";

/** A session storage over a map: Node has none. */
function storing(): void {
  const kept = new Map<string, string>();
  vi.stubGlobal("sessionStorage", {
    getItem: (key: string) => kept.get(key) ?? null,
    setItem: (key: string, value: string) => kept.set(key, value),
    removeItem: (key: string) => kept.delete(key),
    get length() {
      return kept.size;
    },
    key: (at: number) => [...kept.keys()][at] ?? null,
  });
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("drafts kept per tab", () => {
  it("come back under their key, and go once sent", () => {
    storing();
    writeDraft("title:j_a:n_x", { text: "Typed" });
    expect(readDraft("title:j_a:n_x")).toEqual({ text: "Typed" });
    writeDraft("title:j_a:n_x", undefined);
    expect(readDraft("title:j_a:n_x")).toBeUndefined();
  });

  it("lists every draft under a prefix", () => {
    storing();
    writeDraft("rejected:j_a:dates", 1);
    writeDraft("rejected:j_b:dates", 2);
    writeDraft("title:j_a:n_x", 3);
    expect(readDrafts("rejected:")).toEqual([
      ["rejected:j_a:dates", 1],
      ["rejected:j_b:dates", 2],
    ]);
  });

  it("are kept in memory only when the storage refuses", () => {
    vi.stubGlobal("sessionStorage", {
      getItem: () => {
        throw new Error("blocked");
      },
      setItem: () => {
        throw new Error("blocked");
      },
    });
    expect(() => {
      writeDraft("title:j_a:n_x", "kept");
    }).not.toThrow();
    expect(readDraft("title:j_a:n_x")).toBeUndefined();
  });
});

describe("a draft follows its key, not the screen it was typed on", () => {
  it("is empty on another node or journey, and is there again on coming back", () => {
    storing();
    writeDraft("title:j_hiring:n_offer", "Offer, typed");
    const copy = draftAt({ key: "title:j_hiring:n_offer", value: "Offer, typed" }, "title:j_copy:n_offer");
    expect(copy).toEqual({ key: "title:j_copy:n_offer", value: undefined });
    expect(draftAt(copy, "title:j_hiring:n_offer").value).toBe("Offer, typed");
  });
});
