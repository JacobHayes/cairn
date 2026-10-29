// How cards and lines look, as data the components apply (C1, C5, C7): requirements solid with
// an arrowhead, gates dotted with their own end marker, lines in steel and satisfied
// ones grey; not-relevant cards hollow and conditional ones ghosted, "I am here" outlined in ink,
// done work filled green, and an overlay's tags and the one out-of-focus level. Quiet by rule: nothing
// animates or flashes (C6). Pure, so the unit tests read the same decisions the canvas draws.
import type { Card, Line, LineKind } from "./model.ts";
import { standsFor, type CanvasOverlay } from "./overlay.ts";

/** C1: the dash pattern of an implicit edge (dotted), and of a date line (dashed, longer and heavier than the dots so the two never read alike). */
export const DOTTED = "2 3";
export const DATED = "6 4";

/** The marker at a line's target end, by what it stands for: only the end tells a condition from a stage opening. */
export const MARKERS: Record<LineKind, "arrow" | "diamond" | "bar"> = {
  requires: "arrow",
  condition: "diamond",
  stage_opening: "bar",
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
 * a stage opening), steel, waiting or satisfied,
 * lit by an overlay in the colour of its direction, and faded to the one level when an overlay
 * dims what it does not light (or, with no overlay, when `faded`: both its cards are unlit).
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
 * ghosted when conditional, green when done, hollow when skipped), "I am here" (the frontier and active work
 * outlined in ink), the overlay's tone, and the one out-of-focus level: outside the overlay's trace,
 * or `faded`, outside what Frontier or Decisions lights.
 */
export function nodeClasses(card: Card, options: { overlay?: CanvasOverlay | undefined; faded?: boolean } = {}): string[] {
  const { overlay, faded = false } = options;
  const classes = ["node", `node-${card.kind}`];
  const journey = card.journey;
  if (journey?.state === "not_relevant") {
    classes.push("node-not-relevant");
  } else if (journey?.state === "conditional") {
    classes.push("node-conditional");
  } else if (journey?.state === "done") {
    classes.push("node-done");
  } else if (journey?.state === "skipped") {
    classes.push("node-skipped");
  } else if (journey?.state === "snoozed") {
    classes.push("node-snoozed");
  }
  if (journey !== undefined && (journey.here.frontier || journey.here.active)) {
    classes.push("node-here");
  }
  const mark = overlay?.marks[card.key];
  if (mark?.diff !== undefined) {
    // The proposal layer's mark (C14): a fill, an edge whose style is the kind's, and a ring for a choice.
    classes.push("node-diff", `node-diff-${mark.diff}`);
    if (mark.choice === true) {
      classes.push("node-diff-choice");
    }
    if (mark.ghost === true) {
      classes.push("node-ghost");
    }
  } else if (mark !== undefined) {
    classes.push("node-marked", `node-marked-${mark.tone}`);
    if (mark.ghost === true) {
      classes.push("node-ghost");
    }
  }
  // Out of focus is outside the trace or outside the highlight: the lowest opacity wins, so a
  // trace never lifts a card the highlight left unlit (it keeps its mark at 0.3).
  if (faded || (mark === undefined && overlay?.dim === true)) {
    classes.push("node-faded");
  }
  return classes;
}

/**
 * DESIGN, Opacity: the cards out of focus for Depth's Fade (`faded`, the nodes it dims: its own
 * rule already clears a container holding a card in its focus) or for Frontier (`emphasis`, the
 * nodes it lights): a card out of either is dimmed, once. A card stands for the nodes folded into
 * it, so it is in Frontier's focus when any of them is, and so is every container above it. None
 * of either dims nothing.
 */
export function fadedCards(cards: readonly Card[], faded: ReadonlySet<string> | null | undefined, emphasis: ReadonlySet<string> | null | undefined): Set<string> {
  const out = new Set(faded === null || faded === undefined ? [] : cards.filter((card) => faded.has(card.key)).map((card) => card.key));
  if (emphasis !== null && emphasis !== undefined) {
    const parents = new Map(cards.map((card) => [card.key, card.parent]));
    const kept = new Set<string>();
    for (const card of cards.filter((each) => standsFor(each).some((key) => emphasis.has(key)))) {
      for (let at: string | undefined = card.key; at !== undefined && !kept.has(at); at = parents.get(at)) {
        kept.add(at);
      }
    }
    for (const card of cards.filter((each) => !kept.has(each.key))) {
      out.add(card.key);
    }
  }
  return out;
}

/** C5: the words for why a card is on "I am here", in order. */
export function hereWords(card: Card): string[] {
  const here = card.journey?.here;
  if (here === undefined) {
    return [];
  }
  return [
    ...(here.frontier ? ["actionable now"] : []),
    ...(here.active ? ["active"] : []),
    ...(here.mine ? ["yours"] : []),
  ];
}
