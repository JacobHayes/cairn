// ARCHITECTURE, Web UI: drafts survive a reload. What is typed is kept per tab under a key naming
// its journey and node, so it comes back after a reload, goes when sent, and never follows a
// screen reused for another journey or node.
import { afterEach, describe, expect, it, vi } from "vitest";

import { draftAt, draftKeys, readDraft, writeDraft } from "./drafts.ts";
import { memoryStorage } from "./fake.test-support.ts";

function storing(): void {
  vi.stubGlobal("sessionStorage", memoryStorage());
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

  it("are listed by what follows a prefix, so a screen can tell what is unsent", () => {
    storing();
    writeDraft("node-form:route:a:n_two", { edits: {} });
    writeDraft("node-form:route:a:n_one", { edits: {} });
    writeDraft("node-form:route:b:n_other", { edits: {} });
    expect(draftKeys("node-form:route:a:")).toEqual(["n_one", "n_two"]);
    writeDraft("node-form:route:a:n_one", undefined);
    expect(draftKeys("node-form:route:a:")).toEqual(["n_two"]);
  });

  it("are listed when held in memory only, as storage refused them", () => {
    vi.stubGlobal("sessionStorage", Object.assign(memoryStorage(), { setItem: () => { throw new Error("full"); } }));
    writeDraft("node-form:route:c:n_held", { edits: {} });
    expect(draftKeys("node-form:route:c:")).toEqual(["n_held"]);
    expect(readDraft("node-form:route:c:n_held")).toEqual({ edits: {} });
    writeDraft("node-form:route:c:n_held", undefined);
    expect(draftKeys("node-form:route:c:")).toEqual([]);
  });

  it("come back from memory when the storage refuses them, and go once sent", () => {
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
    expect(readDraft("title:j_a:n_x")).toBe("kept");
    writeDraft("title:j_a:n_x", undefined);
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
