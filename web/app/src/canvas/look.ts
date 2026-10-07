// How cards and lines look, as data the components apply (C1, C5, C6, C7): explicit edges
// solid and implicit gates dotted, not-relevant cards grayed and undecided ones ghosted,
// "I am here" outlined, gravity as border weight, and an overlay's dimming. Quiet by rule:
// nothing animates or flashes (C6). Pure, so the unit tests read the same decisions the
// canvas draws.
import type { Card, Line } from "./model.ts";
import type { CanvasOverlay } from "./overlay.ts";

/** C1: the dash pattern of an implicit edge (dotted). */
export const DOTTED = "2 5";

/** How a line is drawn. */
export interface LineLook {
  /** The dash pattern; none for a solid line. */
  dash: string | undefined;
  /** CSS classes for its color and fading. */
  classes: string[];
}

/** C1: a line solid when an explicit edge stands behind it, dotted when every one is implicit. */
export function lineLook(line: Line, overlay?: CanvasOverlay): LineLook {
  const classes = ["line"];
  if (!line.gates) {
    classes.push("line-dates-only");
  }
  if (overlay !== undefined) {
    classes.push(overlay.lines.includes(line.id) ? "line-lit" : overlay.dim ? "line-dim" : "");
  }
  return { dash: line.implicit ? DOTTED : undefined, classes: classes.filter(Boolean) };
}

/**
 * C1, C5, C7: a card's classes: its kind, its relevance look, "I am here" (the frontier and
 * active work outlined; the viewer's own items marked more lightly, since a viewer may own
 * most of a journey), and an overlay's dimming.
 */
export function cardClasses(card: Card, overlay?: CanvasOverlay): string[] {
  const classes = ["card", `card-${card.kind}`];
  const journey = card.journey;
  if (journey?.relevance === "not_relevant") {
    classes.push("card-not-relevant");
  } else if (journey?.relevance === "undecided") {
    classes.push("card-undecided");
  }
  if (journey !== undefined && (journey.here.frontier || journey.here.active)) {
    classes.push("card-here");
  }
  if (journey?.here.mine === true) {
    classes.push("card-mine");
  }
  const mark = overlay?.marks[card.key];
  if (mark !== undefined) {
    classes.push("card-marked", `card-marked-${mark.tone}`);
  } else if (overlay?.dim === true) {
    classes.push("card-dim");
  }
  return classes;
}

/** C5: the words for why a card is on "I am here", in order. */
export function hereWords(card: Card): string[] {
  const here = card.journey?.here;
  if (here === undefined) {
    return [];
  }
  return [
    ...(here.frontier ? ["actionable now"] : []),
    ...(here.startedEarly ? ["active, started early"] : here.active ? ["active"] : []),
    ...(here.mine ? ["yours"] : []),
  ];
}
