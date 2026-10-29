// The Stages list on a route's card, draft or version.
import { valueOf } from "../data/keyed-read.ts";
import { StagesSection } from "../journeys/StagesSection.tsx";
import type { Derived, Shown } from "../screens/route-reads.ts";
import { useRouteSchedule } from "../screens/route-views.ts";
import { routeStagesOf } from "./stages.ts";

export function RouteStages({ shown, framing }: { shown: Derived<Shown> | undefined; framing: boolean }) {
  const read = useRouteSchedule(shown);
  // The list appears once the schedule has answered, so it never reorders under a pointer; a schedule that failed leaves the stages in the route's order, with no days.
  return shown === undefined || read.status === "loading" ? null : <StagesSection stages={routeStagesOf(shown, valueOf(read))} framing={framing} />;
}
