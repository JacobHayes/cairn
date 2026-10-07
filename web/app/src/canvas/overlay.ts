// `CanvasOverlay`: a layer drawn over a canvas's cards and lines without changing them, for
// what a screen wants to point at. Here it is the trace (C7); proposal review (5.7) draws its
// diff with it. A marked card shows its mark; when the overlay dims, unmarked cards and the
// lines it does not light fade, so the marked set stands out without anything moving.
import type { CanvasModel, Card, Trace } from "./model.ts";

export type MarkTone = "accent" | "good" | "warn" | "bad" | "plain";

/** What an overlay says about one card. */
export interface OverlayMark {
  tone: MarkTone;
  label: string;
}

/** A layer over a canvas: marks by card, lines it lights, and what it marks off this canvas. */
export interface CanvasOverlay {
  /** What it shows, for its legend (eg "Trace of Test plan"). */
  title: string;
  marks: Record<string, OverlayMark>;
  /** Line ids it lights; with `dim`, every other line fades. */
  lines: string[];
  /** Fade what it does not mark. */
  dim: boolean;
  /** Nodes it marks that have no card on this canvas (outside the drilled-in container, or with no stand-in). */
  outside: string[];
}

/** C7: the trace's labels: the node traced, its upstream, its downstream, and gravity contributors. */
export const TRACE_LABELS = {
  traced: "traced",
  upstream: "upstream",
  downstream: "downstream",
  contributor: "gravity contributor",
  both: "upstream and downstream",
} as const;

/** What each card stands for: itself and the hidden nodes rolled up into it (C2). */
function standsFor(card: Card): string[] {
  return [card.key, ...card.checklist.map((item) => item.key)];
}

function markOf(card: Card, trace: Trace, sets: { up: Set<string>; down: Set<string>; contributors: Set<string> }): OverlayMark | undefined {
  const keys = standsFor(card);
  if (keys.includes(trace.node)) {
    return { tone: "accent", label: TRACE_LABELS.traced };
  }
  const [up, down] = [keys.some((key) => sets.up.has(key)), keys.some((key) => sets.down.has(key))];
  if (up && down) {
    return { tone: "accent", label: TRACE_LABELS.both };
  }
  if (up) {
    return { tone: "accent", label: TRACE_LABELS.upstream };
  }
  if (down) {
    return keys.some((key) => sets.contributors.has(key))
      ? { tone: "warn", label: TRACE_LABELS.contributor }
      : { tone: "good", label: TRACE_LABELS.downstream };
  }
  return undefined;
}

/**
 * C7: the trace of one node over a canvas: its upstream and downstream (across levels,
 * terminal and not-relevant nodes included) marked on the cards that stand for them, the
 * gravity contributors marked within the downstream, and the lines along either direction.
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
  const downOnly: string[] = [TRACE_LABELS.downstream, TRACE_LABELS.contributor];
  const upward = (key: string) => label(key) !== undefined && !downOnly.includes(label(key) ?? "");
  const downward = (key: string) => label(key) !== undefined && label(key) !== TRACE_LABELS.upstream;
  const lines = model.lines
    .filter((line) => (upward(line.from) && upward(line.to)) || (downward(line.from) && downward(line.to)))
    .map((line) => line.id);
  const drawn = new Set(model.cards.flatMap(standsFor));
  const outside = [...trace.upstream, ...trace.downstream].filter((key) => !drawn.has(key)).sort();
  return { title, marks, lines, dim: true, outside };
}
