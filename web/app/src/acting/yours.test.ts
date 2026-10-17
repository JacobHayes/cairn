// C10, C16: the line for a viewer whose own work is all waiting names what is theirs next and what it waits on.
import { describe, expect, it } from "vitest";

import { actingView } from "./acting.test-support.ts";
import { yoursOf } from "../journeys/figures-model.ts";
import { nextForYou, openForViewer } from "./yours.ts";

const holds = (...nodes: string[]) => nodes.map((node) => ({ node, kinds: ["k_owner"] }));

describe("nextForYou", () => {
  it("is silent while something of theirs is on the frontier", () => {
    expect(nextForYou(actingView(), holds("n_pick", "n_later"))).toBeUndefined();
  });

  it("names their earliest-starting open node and what it waits on", () => {
    const view = actingView();
    // n_later starts Oct 20 after the meeting; n_inner starts Oct 13 behind the stage's opening.
    const next = nextForYou(view, holds("n_later", "n_inner"));
    expect(next).toMatchObject({ node: "n_inner", after: ["Meeting"] });
  });
});

describe("what is open for the viewer", () => {
  // The one rule behind the journey card's count and the line for work that is all waiting.
  it("leaves out a container, work out of scope, finished work and a milestone that reaches itself, in both", () => {
    const view = actingView();
    const found = view.derived.nodes;
    const set = (key: string, change: Record<string, unknown>) => {
      const was = found[key];
      if (was !== undefined) {
        found[key] = { ...was, ...change };
      }
    };
    set("n_later", { auto_reached: true });
    const entries = holds("n_stage", "n_gone", "n_done", "n_later", "n_inner");
    expect(["n_stage", "n_gone", "n_done", "n_later", "n_inner"].filter((key) => openForViewer(view, key))).toEqual(["n_inner"]);
    // The line for work that is all waiting names what is open, never one of the others.
    expect(nextForYou(view, entries)).toMatchObject({ node: "n_inner" });
    // The journey card's count is of the same nodes.
    expect(yoursOf(view, entries).open).toBe(1);
  });
});
