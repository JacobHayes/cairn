// ARCHITECTURE, Web UI: previews. A form's preview is a keyed read: obsolete work posts
// nothing to the derive worker, and the key says what the preview is of and what it is read from.
import { describe, expect, it, vi } from "vitest";

import { FakeDeriver } from "../data/fake.test-support.ts";
import { Skewed } from "../data/host.ts";
import { sameKey, type ReadAt } from "../data/keyed-read.ts";
import { treeOf } from "./graph.ts";
import type { Authored } from "./target.ts";
import { LIVE_PREVIEW, previewKey, type Preview } from "./write.ts";

const graph = { nodes: [] };
const authored: Authored = {
  target: { journey: "j_one" },
  graph,
  tree: treeOf(graph),
  revision: 3,
  deploymentRevision: 1,
  deployment: { revision: 1 },
  today: "2026-10-16",
  lineage: undefined,
  route: undefined,
};
const rename = (title: string) => [{ op: "set_node_field" as const, node: "n_a", value: { title } }];

function readOf(overrides: Partial<ReadAt<Preview>> = {}) {
  const deriver = new FakeDeriver();
  const apply = vi.spyOn(deriver, "apply");
  const key = previewKey(deriver, "u_one", authored, rename("A"));
  const at: ReadAt<Preview> = { held: undefined, signal: new AbortController().signal, compatible: () => true, ...overrides };
  return { apply, read: async () => await LIVE_PREVIEW.read(key.args, at) };
}

describe("a form's preview", () => {
  it("posts nothing to the worker once nobody waits for it", async () => {
    const aborted = new AbortController();
    aborted.abort();
    const { apply, read } = readOf({ signal: aborted.signal });
    await expect(read()).rejects.toMatchObject({ name: "AbortError" });
    expect(apply).not.toHaveBeenCalled();
  });

  it("posts nothing to the worker once the tab is in version skew", async () => {
    const { apply, read } = readOf({ compatible: () => false });
    await expect(read()).rejects.toBeInstanceOf(Skewed);
    expect(apply).not.toHaveBeenCalled();
  });

  it("asks the worker to apply the mutations, and a refusal of the worker is a preview that says so", async () => {
    const { apply, read } = readOf();
    expect(await read()).toMatchObject({ outcome: "unavailable" });
    expect(apply).toHaveBeenCalledOnce();
  });

  it("is of the target, and read again for other mutations, revisions, days or viewers", () => {
    const deriver = new FakeDeriver();
    const base = previewKey(deriver, "u_one", authored, rename("A"));
    expect(sameKey(previewKey(deriver, "u_one", { ...authored }, rename("A")).freshness, base.freshness)).toBe(true);
    for (const other of [
      previewKey(deriver, "u_one", authored, rename("B")),
      previewKey(deriver, "u_one", { ...authored, revision: 4 }, rename("A")),
      previewKey(deriver, "u_one", { ...authored, deploymentRevision: 2 }, rename("A")),
      previewKey(deriver, "u_one", { ...authored, today: "2026-10-17" }, rename("A")),
      previewKey(deriver, "u_two", authored, rename("A")),
    ]) {
      expect(sameKey(other.freshness, base.freshness)).toBe(false);
      expect(sameKey(other.of, base.of)).toBe(true);
    }
    expect(sameKey(previewKey(deriver, "u_one", { ...authored, target: { route: "r_a" } }, rename("A")).of, base.of)).toBe(false);
  });
});
