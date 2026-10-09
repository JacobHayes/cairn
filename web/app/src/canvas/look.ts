// How cards and lines look, as data the components apply (C1, C5, C7): requirements solid with
// an arrowhead, gates dotted with their own end marker, waiting lines in steel and satisfied
// ones grey; not-relevant cards hollow and conditional ones ghosted, "I am here" outlined in ink,
// finished work receding, and an overlay's tags and the one fade level. Quiet by rule: nothing
// animates or flashes (C6). Pure, so the unit tests read the same decisions the canvas draws.
import type { Card, Line, LineKind } from "./model.ts";
import type { CanvasOverlay } from "./overlay.ts";

/** C1: the dash pattern of an implicit edge (dotted), and of a line that only holds dates. */
export const DOTTED = "2 3";

/** The marker at a line's target end, by what it stands for (5.6): only the end tells a condition from a stage opening. */
export const MARKERS: Record<LineKind, "arrow" | "diamond" | "bar" | "none"> = {
  requires: "arrow",
  condition: "diamond",
  stage_opening: "bar",
  dates: "none",
};

/** How a line is drawn. */
export interface LineLook {
  /** The dash pattern; none for a solid line. */
  dash: string | undefined;
  marker: (typeof MARKERS)[LineKind];
  /** CSS classes for its color and fading. */
  classes: string[];
}

/**
 * C1: a line solid when a requirement stands behind it and dotted otherwise (a condition gate,
 * a stage opening, a link that holds dates only), steel while it waits and grey once satisfied,
 * lit by an overlay in the colour of its direction, and faded to the one level when an overlay
 * dims what it does not light (or when `faded`, a filter's).
 */
export function lineLook(line: Line, options: { overlay?: CanvasOverlay | undefined; satisfied?: boolean; faded?: boolean } = {}): LineLook {
  const { overlay, satisfied = false, faded = false } = options;
  const classes = ["line", `line-${line.kind}`, satisfied ? "line-satisfied" : "line-waiting"];
  const lit = overlay?.lines[line.id];
  if (lit !== undefined) {
    classes.push(`line-lit-${lit}`);
  } else if (overlay?.dim === true || (overlay === undefined && faded)) {
    classes.push("line-faded");
  }
  return { dash: line.kind === "requires" ? undefined : DOTTED, marker: MARKERS[line.kind], classes };
}

/**
 * C1, C5, C7: a card's classes: its kind, its display state's look (hollow when not relevant,
 * ghosted when conditional, receding when finished), "I am here" (the frontier and active work
 * outlined in ink), the overlay's tone, and the one fade level.
 */
export function nodeClasses(card: Card, options: { overlay?: CanvasOverlay | undefined; faded?: boolean } = {}): string[] {
  const { overlay, faded = false } = options;
  const classes = ["node", `node-${card.kind}`];
  const journey = card.journey;
  if (journey?.state === "not_relevant") {
    classes.push("node-not-relevant");
  } else if (journey?.state === "conditional") {
    classes.push("node-conditional");
  } else if (journey?.finished === true) {
    classes.push("node-finished");
  }
  if (journey !== undefined && (journey.here.frontier || journey.here.active)) {
    classes.push("node-here");
  }
  const mark = overlay?.marks[card.key];
  if (mark !== undefined) {
    classes.push("node-marked", `node-marked-${mark.tone}`);
  } else if (overlay?.dim === true || (overlay === undefined && faded)) {
    classes.push("node-faded");
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
