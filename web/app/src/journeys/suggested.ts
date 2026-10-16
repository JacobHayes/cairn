// B11: whether completing a journey is suggested, from the engine's status summary and the
// final milestone, kept current as the journey is derived again.
import { useMemo } from "react";

import { useProjected } from "../canvas/hooks.ts";
import { valueOf } from "../data/keyed-read.ts";
import type { Ready } from "../detail/model.ts";
import { completionSuggested } from "./lifecycle.ts";

export function useSuggested(ready: Ready): boolean {
  const summary = valueOf(useProjected(ready, { projection: "status_summary" }));
  return useMemo(() => summary !== undefined && completionSuggested(ready.journey.graph, ready.derived, summary), [ready, summary]);
}
