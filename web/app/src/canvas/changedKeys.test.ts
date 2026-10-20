import { describe, expect, it } from "vitest";

import type { Layer } from "../proposals/layer.ts";
import { changedKeys } from "./JourneyCanvas.tsx";

const layer = (model: object, mode = "reviewing"): Layer => ({ mode, model: { known: true, previewed: true, previewError: undefined, marks: {}, entries: [], unresolved: new Map(), draft: {}, diff: { edgesAdded: [], edgesRemoved: [] }, ...model } }) as unknown as Layer;
const conflict = (node: string) => ({ item: "conflict", conflict: { about: "field", node } });

describe("changedKeys", () => {
  it("lists what waits on a choice first, in the order of the rail's items, then the other changes, then the ends of changed requirements, each once", () => {
    const keys = changedKeys(
      layer({
        // The second conflict's node also has a clean change, so it is marked before the first's.
        entries: [{ key: "n_a" }, { key: "n_d" }, { key: "n_b" }],
        draft: { items: [conflict("n_b"), conflict("n_d")] },
        unresolved: new Map([[0, "unresolved"], [1, "unresolved"]]),
        diff: { edgesAdded: [{ node: "n_a", requires: "n_c" }], edgesRemoved: [] },
      }),
    );
    expect(keys).toEqual(["n_b", "n_d", "n_a", "n_c"]);
  });

  it("holds the first fit back only while the destination after is still being derived", () => {
    expect(changedKeys(layer({ known: false, previewed: false }))).toEqual([]);
    // A preview that finished without a destination after (an invalid proposal) is not waited for.
    expect(changedKeys(layer({ known: false, previewed: true }))).toBeUndefined();
  });
});
