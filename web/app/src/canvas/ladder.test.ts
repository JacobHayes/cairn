// Depth's ladder (C2): the step and the viewer's own clicks decide what is open.
import { expect, test } from "vitest";

import type { GraphNode } from "../detail/model.ts";
import { collapsedAt, leftOutAt, stepOf, stepsFor, withStep } from "./ladder.ts";

const node = (key: string, kind: GraphNode["kind"], parent?: string): GraphNode => ({ key, id: key, kind, title: key, ...(parent === undefined ? {} : { parent }) });
const nodes = [node("s1", "group"), node("s1g", "group", "s1"), node("s1a", "action", "s1g"), node("s2", "group"), node("s3", "group")];

test("at Stages the stage holding the frontier is open and the rest collapsed; the viewer's clicks win until a step is picked", () => {
  const current = new Set(["s1"]);
  expect(collapsedAt("stages", nodes, current, [], [])).toEqual(["s2", "s3"]);
  expect(collapsedAt("stages", nodes, current, ["s2"], ["s1"])).toEqual(["s1", "s3"]);
  expect(collapsedAt("deliverables", nodes, current, [], ["s1g"])).toEqual(["s1g"]);
  expect(collapsedAt("deliverables", nodes, current, [], ["s1g", "removed"])).toEqual(["s1g"]);
  expect(withStep({ step: undefined, open: ["a"], shut: ["b"] }, "deliverables")).toEqual({ step: "deliverables", open: [], shut: [] });
  // Stages leaves out a top-level decision and what is beneath it, never a stage, a milestone or a node inside a stage.
  const mixed = [...nodes, node("d", "decision"), node("da", "action", "d"), node("m", "milestone"), node("sd", "decision", "s1")];
  expect([...leftOutAt("stages", mixed)].sort()).toEqual(["d", "da"]);
  expect(leftOutAt("deliverables", mixed).size).toBe(0);
});

test("a view opens at Actions, and has no Stages rung without stages", () => {
  expect(stepOf({ step: undefined }, nodes)).toBe("actions");
  expect(stepOf({ step: "stages" }, nodes)).toBe("stages");
  const flat = [node("d", "decision"), node("a", "action")];
  expect(stepOf({ step: "stages" }, flat)).toBe("actions");
  expect(stepOf({ step: "deliverables" }, flat)).toBe("deliverables");
  expect(stepsFor(flat)).not.toContain("stages");
  expect(stepsFor(nodes)).toContain("stages");
});
