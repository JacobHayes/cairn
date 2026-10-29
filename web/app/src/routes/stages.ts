// A route's Stages list: its top-level groups in the order the work runs (ordered as the timeline orders its rows), each with the days it spans. A route has no
// state, progress or due date: only what the route defines.
import type { RouteSchedule } from "@cairn/wasm";

import { treeOf } from "../authoring/graph.ts";
import { isStage } from "../detail/model.ts";
import type { StageRowData } from "../journeys/StagesSection.tsx";
import type { Shown } from "../screens/route-reads.ts";
import { nodeTitle } from "../ui/gone.ts";
import { byReach, reachesOf, routeWords } from "../timeline/rows.ts";

/** The graph's stages as rows; a stage's days are the span of what it holds (the timeline's), and unknown until the schedule is. */
export function routeStagesOf(shown: Pick<Shown, "graph">, schedule: RouteSchedule | undefined): StageRowData[] {
  const tree = treeOf(shown.graph);
  const reachOf = reachesOf(tree, new Map((schedule?.entries ?? []).map((entry) => [entry.node, entry])));
  const byDays = byReach(tree, reachOf);
  return (shown.graph.nodes ?? [])
    .filter(isStage)
    .toSorted((one, other) => byDays(one.key, other.key))
    .map((node) => {
      const reach = reachOf(node.key);
      return { key: node.key, title: nodeTitle(node.title), due: reach === undefined ? undefined : routeWords(reach), dueInk: "mono" };
    });
}
