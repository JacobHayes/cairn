// What a route's three views read (C2, C13): the level of the shown graph for the page's view
// state, with the node the address names always drawn (the one rule a journey's views use,
// canvas/hooks.ts), and its schedule of earliest days. A route has no state, so the level is
// derived from the graph alone, on today's date.
import type { RouteSchedule } from "@cairn/wasm";

import { useLevelBy, type LevelRead } from "../canvas/hooks.ts";
import { type ReadView } from "../data/keyed-read.ts";
import { useDeployment, useKeyedRead, useSession, useToday } from "../data/react.ts";
import type { LevelRequest } from "../journeys/requests.ts";
import { nodesOf, ROUTE_LEVEL, ROUTE_SCHEDULE, routeLevelKey, routeScheduleKey, type Shown } from "./route-reads.ts";

/** The level of `shown` that `request` asks for, with `selected` always drawn (C7), and the keys it draws. */
export function useRouteLevel(shown: Shown, request: LevelRequest | undefined, selected?: string): LevelRead {
  const { deriver } = useSession();
  const deployment = useDeployment();
  const day = useToday();
  return useLevelBy((asked) => useKeyedRead(ROUTE_LEVEL, routeLevelKey(deriver, shown, asked, deployment, day)).view, nodesOf(shown), request, selected);
}

/** The earliest days of `shown`'s nodes from its start (C13). */
export function useRouteSchedule(shown: Shown): ReadView<RouteSchedule> {
  const { deriver } = useSession();
  const deployment = useDeployment();
  return useKeyedRead(ROUTE_SCHEDULE, routeScheduleKey(deriver, shown, deployment)).view;
}
