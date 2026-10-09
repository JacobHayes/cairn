// `CanvasOverlay`: a layer drawn over a canvas's cards and lines without changing them, for
// what a screen wants to point at. Here it is the trace (C7), which selecting a node draws;
// proposal review (5.7) draws its diff with it. A marked card shows its tag; when the overlay
// dims, unmarked cards and the lines it does not light fade to the one fade level, so the marked
// set stands out without anything moving.
import type { CanvasModel, Card, Trace } from "./model.ts";

export type MarkTone = "ink" | "accent" | "good" | "warn" | "bad" | "plain";

/** What an overlay says about one card. */
export interface OverlayMark {
  tone: MarkTone;
  /** The tag it hangs, and what tests read. */
  label: string;
  /** More to say of the card, on the tag's hover (a diff's conflict or orphan). */
  note?: string;
  /** A gravity contributor: a dot by its state chip (5.7). */
  contributor?: true;
  /** The tag is not drawn: the traced card itself, which the selection already shows. */
  quiet?: true;
  /** The card is going away: kept where it was, struck and ghosted (a proposal's removal). */
  ghost?: true;
}

/** How a line is lit: ink (upstream) or accent (downstream), faint when it is a finished upstream line, or lit for a diff. */
export type LineMark = "ink" | "accent" | "faint" | "lit";

/** A layer over a canvas: marks by card, lines it lights, and what it marks off this canvas. */
export interface CanvasOverlay {
  /** What it shows, for its legend (eg "Trace of Test plan"). */
  title: string;
  marks: Record<string, OverlayMark>;
  /** Line ids it lights, and how; with `dim`, every other line fades. */
  lines: Record<string, LineMark>;
  /** Fade what it does not mark. */
  dim: boolean;
  /** Nodes it marks that have no card on this canvas (outside the view, or with no stand-in). */
  outside: string[];
  /** Its tags keep their size on screen at any zoom (a diff's words must be readable from far out). */
  screenTags?: true;
}

/** C7: the trace's labels: the node traced, what it needs, what it unblocks. */
export const TRACE_LABELS = {
  traced: "selected",
  needs: "needs",
  unblocks: "unblocks",
  both: "needs and unblocks",
} as const;

/** What each card stands for: itself and the hidden nodes rolled up into it (C2). */
export function standsFor(card: Card): string[] {
  return [card.key, ...card.rolledUp];
}

function markOf(card: Card, trace: Trace, sets: { up: Set<string>; down: Set<string>; contributors: Set<string> }): OverlayMark | undefined {
  const keys = standsFor(card);
  if (keys.includes(trace.node)) {
    return { tone: "accent", label: TRACE_LABELS.traced, quiet: true };
  }
  const [up, down] = [keys.some((key) => sets.up.has(key)), keys.some((key) => sets.down.has(key))];
  const contributor = down && keys.some((key) => sets.contributors.has(key));
  if (up && down) {
    return { tone: "accent", label: TRACE_LABELS.both };
  }
  if (up) {
    return { tone: "ink", label: TRACE_LABELS.needs };
  }
  if (down) {
    return { tone: "accent", label: TRACE_LABELS.unblocks, ...(contributor ? { contributor: true as const } : {}) };
  }
  return undefined;
}

/**
 * C7: the trace of one node over a canvas: its upstream ("needs", in ink) and downstream
 * ("unblocks", in the accent), terminal and not-relevant nodes included, marked on the cards that
 * stand for them, with the gravity contributors dotted and the lines along either direction (a
 * finished upstream line faint, so what is still in the way stands out).
 */
export function traceOverlay(trace: Trace, model: CanvasModel, title: string): CanvasOverlay {
  const sets = { up: new Set(trace.upstream), down: new Set(trace.downstream), contributors: new Set(trace.gravity_contributors) };
  const marks: Record<string, OverlayMark> = {};
  for (const card of model.cards) {
    const mark = markOf(card, trace, sets);
    if (mark !== undefined) {
      marks[card.key] = mark;
    }
  }
  const label = (key: string) => marks[key]?.label;
  const upward = (key: string) => label(key) === TRACE_LABELS.needs || label(key) === TRACE_LABELS.traced || label(key) === TRACE_LABELS.both;
  const downward = (key: string) => label(key) === TRACE_LABELS.unblocks || label(key) === TRACE_LABELS.traced || label(key) === TRACE_LABELS.both;
  const lines: Record<string, LineMark> = {};
  for (const line of model.lines) {
    if (upward(line.from) && upward(line.to)) {
      lines[line.id] = line.satisfied === true ? "faint" : "ink";
    } else if (downward(line.from) && downward(line.to)) {
      lines[line.id] = "accent";
    }
  }
  const drawn = new Set(model.cards.flatMap(standsFor));
  const outside = [...trace.upstream, ...trace.downstream].filter((key) => !drawn.has(key)).sort();
  return { title, marks, lines, dim: true, outside };
}

/**
 * C14: marks by node drawn over a canvas (proposal review's diff): each card shows the mark of
 * the node it stands for, or of one rolled up into it; lines into or out of a marked card are
 * lit; nothing dims unless `dim` (a filter is on), so what a change leaves alone stays readable.
 * A marked node with no card is listed as outside.
 */
export function marksOverlay(title: string, marks: Record<string, OverlayMark>, model: CanvasModel, dim = false): CanvasOverlay {
  const shown: Record<string, OverlayMark> = {};
  for (const card of model.cards) {
    const key = standsFor(card).find((each) => marks[each] !== undefined);
    const mark = key === undefined ? undefined : marks[key];
    if (mark !== undefined) {
      shown[card.key] = mark;
    }
  }
  const lines = Object.fromEntries(model.lines.filter((line) => shown[line.from] !== undefined || shown[line.to] !== undefined).map((line) => [line.id, "lit" as const]));
  const drawn = new Set(model.cards.flatMap(standsFor));
  const outside = Object.keys(marks).filter((key) => !drawn.has(key)).sort();
  return { title, marks: shown, lines, dim, outside, screenTags: true };
}
