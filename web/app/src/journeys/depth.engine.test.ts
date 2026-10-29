// C2 against the real engine: every Depth step is a level request, so what a step hides rolls up
// into its container and a node that waits on it carries the hidden-prerequisites marker. The
// tab never hides a node on its own, so the marker holds at every step on every view.
import type { Schema } from "@cairn/client";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { journeyAfter, seeded, type Fixtures } from "../authoring/engine.test-support.ts";
import { collapsedAt, drawnStep } from "../canvas/ladder.ts";
import { STEPS, type Step } from "../canvas/model.ts";
import type { GraphNode } from "../detail/model.ts";
import { levelRequestOf } from "./requests.ts";
import { DEFAULT_VIEW_STATE } from "./view.ts";

const JOURNEY = "j_vendor_eval";

describe("the hidden-prerequisites marker at every depth", () => {
  let host: Fixtures;
  let level: (depth: Step, fade: boolean) => Schema<"Level">;
  let free: () => void;
  beforeAll(async () => {
    host = await seeded();
    // A top-level action nothing hides at Actions, and a top-level milestone that requires it.
    const text = journeyAfter(host, JOURNEY, [
      { op: "add_node", node: { key: "n_depth_action", id: "depth_action", kind: "action", title: "A loose action" } },
      { op: "add_node", node: { key: "n_depth_milestone", id: "depth_milestone", kind: "milestone", title: "Everything agreed" } },
      { op: "add_edge", edge: { node: "n_depth_milestone", requires: "n_depth_action" } },
    ]);
    const derived = host.engine.derive(text);
    free = () => { derived.free(); };
    const nodes = (JSON.parse(text) as Schema<"DomainDocument">).journey.graph.nodes as GraphNode[];
    level = (depth, fade) => derived.project(levelRequestOf({ ...DEFAULT_VIEW_STATE, fade }, drawnStep(depth, fade), collapsedAt(drawnStep(depth, fade), nodes, new Set(), [], [])));
  });

  afterAll(() => {
    free();
  });

  it.each(STEPS.filter((step) => step !== "actions"))("%s: the action is rolled up out of the level and the milestone says what it waits on", (depth) => {
    const hidden = level(depth, false);
    expect(hidden.nodes.some((node) => node.key === "n_depth_action")).toBe(false);
    expect(hidden.nodes.find((node) => node.key === "n_depth_milestone")?.hidden_prerequisites).toEqual(["n_depth_action"]);
  });

  it("Actions draws the action as its own node and the milestone has no marker", () => {
    const drawn = level("actions", false);
    expect(drawn.nodes.some((node) => node.key === "n_depth_action")).toBe(true);
    expect(drawn.nodes.find((node) => node.key === "n_depth_milestone")?.hidden_prerequisites ?? []).toEqual([]);
  });

  it.each(STEPS)("%s with Fade: the level is the Actions one, so every node is drawn and none is marked", (depth) => {
    expect(level(depth, true)).toEqual(level("actions", false));
  });
});
