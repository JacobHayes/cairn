// The detail ladder (C2, 5.1): the step and the viewer's own clicks decide what is open.
import { expect, test } from "vitest";

import type { GraphNode } from "../detail/model.ts";
import { collapsedAt, currentStages, leftOutAt, withStep } from "./ladder.ts";

const node = (key: string, kind: GraphNode["kind"], parent?: string): GraphNode => ({ key, id: key, kind, title: key, ...(parent === undefined ? {} : { parent }) });
const nodes = [node("s1", "group"), node("s1g", "group", "s1"), node("s1a", "action", "s1g"), node("s2", "group"), node("s3", "group")];

test("at Stages the stage holding the frontier is open and the rest collapsed; the viewer's clicks win until a step is picked", () => {
  const current = currentStages(nodes, ["s1a"], []);
  expect([...current]).toEqual(["s1"]);
  expect(collapsedAt("stages", nodes, current, [], [])).toEqual(["s2", "s3"]);
  expect(collapsedAt("stages", nodes, current, ["s2"], ["s1"])).toEqual(["s1", "s3"]);
  expect(collapsedAt("work", nodes, current, [], ["s1g"])).toEqual(["s1g"]);
  expect(withStep({ step: undefined, open: ["a"], shut: ["b"] }, "work")).toEqual({ step: "work", open: [], shut: [] });
  // Stages leaves out a top-level decision and what is beneath it, never a stage, a milestone or a node inside a stage.
  const mixed = [...nodes, node("d", "decision"), node("da", "action", "d"), node("m", "milestone"), node("sd", "decision", "s1")];
  expect([...leftOutAt("stages", mixed)].sort()).toEqual(["d", "da"]);
  expect(leftOutAt("work", mixed).size).toBe(0);
});
