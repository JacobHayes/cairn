// The Stages list on a route's card, draft or version.
import type { RouteSchedule } from "@cairn/wasm";

import { valueOf, type ReadView } from "../data/keyed-read.ts";
import { StagesSection } from "../journeys/StagesSection.tsx";
import type { Derived, Shown } from "../screens/route-reads.ts";
import { routeStagesOf } from "./stages.ts";

export function RouteStages({ shown, schedule: read }: { shown: Derived<Shown> | undefined; schedule: ReadView<RouteSchedule> }) {
  // The list appears once the schedule has answered, so it never reorders under a pointer; a schedule that failed leaves the stages in the route's order, with no days.
  return shown === undefined || read.status === "loading" ? null : <StagesSection stages={routeStagesOf(shown, valueOf(read))} note="Days count from the start of the journey." />;
}
