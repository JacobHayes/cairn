// A route's Stages list: its top-level groups in the order the work runs (by the earliest day the
// engine's schedule starts each and then finishes it, as the timeline orders its rows; the stages
// with none after, then as the route lists them), each with the days it spans. A route has no
// state, progress or due date: only what the route defines.
import type { RouteSchedule } from "@cairn/wasm";

import { treeOf } from "../authoring/graph.ts";
import { isStage } from "../detail/model.ts";
import type { StageRowData } from "../journeys/StagesSection.tsx";
import type { Shown } from "../screens/route-reads.ts";
import { nodeTitle } from "../ui/gone.ts";
import { reachesOf, routeWords } from "../timeline/rows.ts";

/** The graph's stages as rows; a stage's days are the span of what it holds (the timeline's), and unknown until the schedule is. */
export function routeStagesOf(shown: Pick<Shown, "graph">, schedule: RouteSchedule | undefined): StageRowData[] {
  const tree = treeOf(shown.graph);
  const reachOf = reachesOf(tree, new Map((schedule?.entries ?? []).map((entry) => [entry.node, entry])));
  const rows = (shown.graph.nodes ?? []).filter(isStage).map((node) => ({ node, reach: reachOf(node.key) }));
  // The sort is stable, so equal days keep the route's order.
  const from = (reach: { from: number } | undefined) => reach?.from ?? Number.MAX_SAFE_INTEGER;
  const to = (reach: { to: number } | undefined) => reach?.to ?? Number.MAX_SAFE_INTEGER;
  return rows
    .toSorted((one, other) => from(one.reach) - from(other.reach) || to(one.reach) - to(other.reach))
    .map(({ node, reach }) => ({ key: node.key, title: nodeTitle(node.title), due: reach === undefined ? undefined : routeWords(reach) }));
}
