// B6: where a snooze ends is said one way on every surface, and a node that was removed is never
// named by its key.
import { describe, expect, it } from "vitest";

import { testView } from "../detail/view.test-support.ts";
import { NODE_GONE } from "../ui/gone.ts";
import { snoozeEnd, snoozeEndWords, snoozeWords } from "./snooze.ts";
import { dateWords } from "./when.ts";

describe("where a snooze ends", () => {
  const view = testView();
  const [first] = view.journey.graph.nodes ?? [];

  it("is a date in the app's date words, or a node by its title with what it waits for", () => {
    expect(snoozeEnd(view, { date: "2026-10-20" })).toEqual({ name: dateWords("2026-10-20", view.derived.today), tail: "" });
    const node = snoozeEnd(view, { node: first?.key ?? "" });
    expect(node.name).toBe(first?.title);
    expect(node.tail).not.toBe("");
    expect(snoozeEndWords(view, { node: first?.key ?? "" })).toBe(`${node.name}${node.tail}`);
  });

  it("names a removed node as a removed item, never its key", () => {
    const words = snoozeEndWords(view, { node: "n_removed_long_ago" });
    expect(words).toContain(NODE_GONE);
    expect(words).not.toContain("n_removed_long_ago");
  });

  it("is said for a node held by its container as the container's title", () => {
    const [node, holder] = view.journey.graph.nodes ?? [];
    const derived = { ...view.derived.nodes[node?.key ?? ""], snoozed_via: holder?.key } as Parameters<typeof snoozeWords>[1];
    const held = snoozeWords(view, derived);
    expect(held?.words).toContain(holder?.title);
    expect(held?.tail).toBeUndefined();
  });
});
