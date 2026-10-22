// What the tour shows now, as a plain value over the machine's state and the person's location:
// the rule for where it opens (Home only, once the name prompt is out of the way, never over a
// deep link) and for what stands in its place while a run is on. Pure, so the rule is tested
// without a page.
import type { TourSnapshot } from "./machine.ts";
import { underJourney } from "./where.ts";

export type Phase = "busy" | "idle" | "running" | "ending" | "failed" | "ended";

/** The machine's state as the screens tell it apart. */
export function phaseOf(snapshot: TourSnapshot): Phase {
  if (snapshot.matches("idle")) {
    return "idle";
  }
  if (snapshot.matches("running")) {
    return "running";
  }
  if (snapshot.matches("ending")) {
    return "ending";
  }
  if (snapshot.matches("failed")) {
    return "failed";
  }
  return snapshot.matches("ended") ? "ended" : "busy";
}

/** What stands over the workspace: a panel with no target, the overlay of a step, or one line saying the tour is paused. */
export type Surface = "none" | "welcome" | "continue" | "start-failed" | "cleanup-failed" | "end" | "overlay" | "paused";

export interface SurfaceInput {
  phase: Phase;
  /** What failed, in the failed phase. */
  failure: "start" | "cleanup" | undefined;
  /** The record offers the tour: none, or a started run whose journey is gone. */
  offerable: boolean;
  /** The person asked for it (the rail, You). */
  invited: boolean;
  /** A closing write did not land: this tab keeps the offer shut. */
  unsaved: boolean;
  /** The practice journey of the run in hand. */
  run: string | undefined;
  pathname: string;
  home: boolean;
  /** The caller and the deployment are read and the name prompt is not asking. */
  nameSettled: boolean;
}

/**
 * What to show. The welcome opens on Home only, and only once the name prompt is out of the
 * way, so what a person came to a link for is never covered and the tour never competes with
 * the first question. A started run shows its step over the practice journey and one line
 * anywhere else, so practice instructions never sit over real work.
 */
export function surfaceOf(input: SurfaceInput): Surface {
  const { phase, home } = input;
  switch (phase) {
    case "busy":
    case "ending":
      return "none";
    case "failed":
      return home ? (input.failure === "cleanup" ? "cleanup-failed" : "start-failed") : "none";
    case "ended":
      return home ? "end" : "none";
    case "running":
      if (underJourney(input.pathname, input.run)) {
        return "overlay";
      }
      if (home) {
        return input.nameSettled ? "continue" : "none";
      }
      return "paused";
    case "idle":
      return home && input.nameSettled && ((input.offerable && !input.unsaved) || input.invited) ? "welcome" : "none";
  }
}

/** Whether the tour is open to a person: a record with no ending, or a run that is on. The rail's link is for this. */
export function eligible(phase: Phase, offerable: boolean, unsaved: boolean): boolean {
  return phase === "running" || (phase === "idle" && offerable && !unsaved);
}
