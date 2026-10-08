// G1, G2: a note or link's content and its artifact designation; G3: a rendered draft's copy
// text; J4: history pages joined where a patch spans two.
import type { Schema } from "@cairn/client";
import { describe, expect, it } from "vitest";

import { contentOf, designated, newAttachmentKey, withContent } from "./Attachments.tsx";
import { Reads, appended } from "./History.tsx";
import { draftText, journeyUrl } from "./Resources.tsx";

describe("annotations (G1, G2)", () => {
  const link: Schema<"AnnotationBody"> = { key: "a_one", node: "n_a", title: "Draft", reference: "https://example.org/r" };

  it("reads a link's type and address, and a note's text", () => {
    expect(contentOf(link)).toEqual({ type: "reference", text: "https://example.org/r" });
    expect(contentOf({ key: "a_two", note: "Hello" })).toEqual({ type: "note", text: "Hello" });
  });

  it("designates a link as the artifact and back, keeping its key, node, title, and address", () => {
    const artifact = designated(link, true);
    expect(artifact).toEqual({ key: "a_one", node: "n_a", title: "Draft", artifact: "https://example.org/r" });
    expect(designated(artifact, false)).toEqual(link);
  });

  it("writes one content field, of the type chosen", () => {
    const note = withContent(link, "note", "Now a note");
    expect(contentOf(note)).toEqual({ type: "note", text: "Now a note" });
    expect(note).not.toHaveProperty("reference");
  });

  it("mints keys the schema accepts, fresh each time", () => {
    const key = newAttachmentKey();
    expect(key).toMatch(/^a_[a-z0-9][a-z0-9_-]*$/);
    expect(key).not.toBe(newAttachmentKey());
  });
});

describe("message drafts (G3)", () => {
  it("copies text with each missing placeholder written as a marker", () => {
    const text = draftText({ segments: [{ text: "Hi " }, { missing: "roles.owner.name" }, { text: "." }] });
    expect(text).toBe("Hi [missing: roles.owner.name].");
  });

  it("links to the journey's address on this origin", () => {
    expect(journeyUrl("j_one", "https://cairn.example")).toBe("https://cairn.example/journeys/j_one");
  });
});

describe("history pages (J4)", () => {
  const event = (patch: string, ordinal: number) =>
    ({ patch_id: patch, ordinal }) as unknown as Schema<"Event">;

  it("joins a patch split across two pages", () => {
    const shown = [{ patch_id: "p_a", events: [event("p_a", 0)] }];
    const page = { patches: [{ patch_id: "p_a", events: [event("p_a", 1)] }, { patch_id: "p_b", events: [event("p_b", 0)] }] };
    const joined = appended(shown, page);
    expect(joined.map((patch) => [patch.patch_id, patch.events.length])).toEqual([
      ["p_a", 2],
      ["p_b", 1],
    ]);
  });
});

describe("history reads (J4, H6)", () => {
  it("shows only the latest read's answer, whatever order they answer in", () => {
    const reads = new Reads();
    const first = reads.start();
    const second = reads.start();
    expect(reads.current(first)).toBe(false);
    expect(reads.current(second)).toBe(true);
  });
});
