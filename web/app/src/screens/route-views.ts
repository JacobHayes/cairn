// What a route's views read: its shown graph derived once in the worker (route-reads.ts), and
// from that derivation the level, the trace and the schedule of earliest days (C2, C7, C13), by
// the hooks a journey's views use (canvas/hooks.ts). A route has no state, so nothing it
// answers depends on a day.
import type { RouteSchedule } from "@cairn/wasm";

import { PROJECTION, valueOf, type ReadKey, type ReadSource, type ReadView } from "../data/keyed-read.ts";
import { useDeployment, useKeyedRead, useSession } from "../data/react.ts";
import type { Deriver } from "../data/host.ts";
import { useReviewing } from "../proposals/layer.ts";
import { ROUTE_DERIVED, routeDerivedKey, type Derived, type Shown } from "./route-reads.ts";

/** `shown` derived and held, as the last derivation of it that arrived (none before the first). */
export function useRouteDerived(shown: Shown | undefined): ReadView<Derived<Shown>> {
  const { deriver } = useSession();
  const deployment = useDeployment();
  return useKeyedRead(ROUTE_DERIVED, routeDerivedKey(deriver, shown, deployment)).view;
}

interface ScheduleArgs {
  deriver: Deriver;
  slot: string;
}

const ROUTE_SCHEDULE: ReadSource<ScheduleArgs, RouteSchedule> = {
  policy: PROJECTION,
  read: ({ deriver, slot }) => deriver.routeSchedule(slot),
};

/** The key of a derived graph's schedule: what it is of is what the derivation reads are of; it is read again for each derivation. */
export function routeScheduleKey(deriver: Deriver, derived: Derived<Shown> | undefined): ReadKey<ScheduleArgs> | undefined {
  return derived === undefined ? undefined : { of: [derived.reads], freshness: [JSON.stringify(derived.key)], args: { deriver, slot: derived.key.journey } };
}

/** The earliest days of the derived graph's nodes from its start (C13). */
export function useRouteSchedule(derived: Derived<Shown> | undefined): ReadView<RouteSchedule> {
  const { deriver } = useSession();
  return useKeyedRead(ROUTE_SCHEDULE, routeScheduleKey(deriver, derived)).view;
}

/**
 * C14: under a proposal's layer that removes nodes, the route as it is, derived, which supplies
 * what the proposal removes, put back where it was; `waiting` while it is being derived.
 */
export function useRouteBefore(): { before: Derived<Shown> | undefined; waiting: boolean } {
  const layer = useReviewing();
  const wanted = layer?.removed.size === 0 ? undefined : layer?.beforeShown;
  const read = useRouteDerived(wanted);
  const before = valueOf(read);
  return { before, waiting: wanted !== undefined && before === undefined && read.status !== "failed" };
}
