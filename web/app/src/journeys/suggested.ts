// B11: whether completing a journey is suggested, as the engine's status summary says, and how
// much of the journey is still in scope and unfinished, kept current as the journey is derived again.
import { useProjected } from "../canvas/hooks.ts";
import { valueOf } from "../data/keyed-read.ts";
import type { Ready } from "../detail/model.ts";

export function useSuggested(ready: Ready): { remaining: number } | undefined {
  const summary = valueOf(useProjected(ready, { projection: "status_summary" }));
  return summary?.completion_suggested === true ? { remaining: summary.remaining } : undefined;
}
