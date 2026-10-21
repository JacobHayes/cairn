// `CanvasOverlay`: a layer drawn over a canvas's cards and lines without changing them, for
// what a screen wants to point at. Here it is the trace (C7), which selecting a node draws, and
// the proposal layer's marks (C14), which the cards draw as a fill, an edge and a word in their
// own kind line. A traced card shows its tag; when the overlay dims, unmarked cards and the lines
// it does not light fade to the one fade level, so the marked set stands out without anything moving.
import type { DiffKind, DiffMark } from "../proposals/model.ts";
import type { CanvasModel, Card, Line, Trace } from "./model.ts";

export type MarkTone = "ink" | "accent" | "good" | "warn" | "bad" | "plain";

/** A requirement: `node` needs `requires`. */
export interface RequirementEdge {
  node: string;
  requires: string;
}

/** The requirements a diff adds and removes. */
export interface EdgeChanges {
  added: readonly RequirementEdge[];
  removed: readonly RequirementEdge[];
}

/** What an overlay says about one card. */
export interface OverlayMark {
  tone: MarkTone;
  /** The tag it hangs, and what tests read. */
  label: string;
  /** More to say of the card, on the tag's hover (a diff's conflict or orphan). */
  note?: string;
  /** A gravity contributor (an UNBLOCKS tag): a dot leads the tag, whose hover says it adds to the selected node's gravity. */
  contributor?: true;
  /** The tag is not drawn: the traced card itself, which the selection already shows. */
  quiet?: true;
  /** The card is going away: kept where it was, struck and ghosted (a proposal's removal). */
  ghost?: true;
  /** The proposal layer's mark (C14): the card is added, changed or removed, drawn as a fill, an edge and the word in its kind line, never a tag. */
  diff?: DiffKind;
  /** The layer's mark: an unresolved choice is about the card. */
  choice?: true;
}

/** How a line is lit: ink (upstream) or accent (downstream), lit for a card a diff marks, or add or remove for a requirement the diff adds or removes. */
export type LineMark = "ink" | "accent" | "lit" | "add" | "remove";

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
 * C7: the trace of one node over a canvas, its tags and its lines. A line is upstream (ink) when
 * its source is needed and its target is needed or holds something needed (the node itself
 * included, so the containers around it count); downstream (accent) when its target is unblocked
 * and its source is unblocked or holds something unblocked; any other line rests. Rolled-up ends
 * follow their card's marks, and a line's kind or finished state never matters.
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
  // A card carries a side of the trace when it, or a card inside it, is on that side; the traced
  // card is on both, so the containers around it carry both.
  const parentOf = new Map(model.cards.map((card) => [card.key, card.parent]));
  const carriers = (onSide: (key: string) => boolean) => {
    const carrying = new Set<string>();
    for (const card of model.cards.filter((each) => onSide(each.key))) {
      for (let at: string | undefined = card.key; at !== undefined && !carrying.has(at); at = parentOf.get(at)) {
        carrying.add(at);
      }
    }
    return carrying;
  };
  const [upCarriers, downCarriers] = [carriers(upward), carriers(downward)];
  const lines: Record<string, LineMark> = {};
  for (const line of model.lines) {
    if (label(line.from) === TRACE_LABELS.traced && label(line.to) === TRACE_LABELS.traced) {
      continue;
    }
    if (upward(line.from) && upCarriers.has(line.to)) {
      lines[line.id] = "ink";
    } else if (downCarriers.has(line.from) && downward(line.to)) {
      lines[line.id] = "accent";
    }
  }
  const drawn = new Set(model.cards.flatMap(standsFor));
  const outside = [...trace.upstream, ...trace.downstream].filter((key) => !drawn.has(key)).sort();
  return { title, marks, lines, dim: true, outside };
}

/** The tone a kind of change takes on a line or a chip. */
const TONES: Record<DiffKind, MarkTone> = { added: "good", changed: "warn", removed: "bad" };

/** The words of each kind of change. */
const WORDS: Record<DiffKind, string> = { added: "Added", changed: "Changed", removed: "Removed" };

/** What the layer changes on a canvas that the marks alone do not say: the removed requirements' lines, and the kept cards that lost nodes rolled up into them. */
export interface LayerDrawn {
  removedLines?: ReadonlySet<string>;
  shrunk?: ReadonlyMap<string, string[]>;
}

/**
 * C14: the proposal layer's marks over a canvas: each card shows the mark of the node it stands
 * for, or of one rolled up into it (a kept card that lost nodes rolled up into it is changed);
 * lines into or out of a marked card are lit, a requirement the proposal adds is drawn as added
 * and one it removes as removed. Nothing dims, so what a change leaves alone stays readable. A
 * marked node with no card is listed as outside.
 */
export function marksOverlay(title: string, marks: Record<string, DiffMark>, model: CanvasModel, edges: EdgeChanges = { added: [], removed: [] }, drawn: LayerDrawn = {}): CanvasOverlay {
  const shown: Record<string, OverlayMark> = {};
  for (const card of model.cards) {
    const key = standsFor(card).find((each) => marks[each] !== undefined);
    const mark = key === undefined ? undefined : marks[key];
    const lost = drawn.shrunk?.get(card.key);
    if (mark !== undefined) {
      shown[card.key] = { tone: TONES[mark.kind], label: WORDS[mark.kind], diff: mark.kind, ...(mark.choice === true ? { choice: true as const } : {}), ...(mark.note === undefined ? {} : { note: mark.note }), ...(mark.kind === "removed" ? { ghost: true as const } : {}) };
    } else if (lost !== undefined) {
      shown[card.key] = { tone: "warn", label: WORDS.changed, diff: "changed", note: `${String(lost.length)} inside it removed` };
    }
  }
  const cardOf = new Map(model.cards.map((card) => [card.key, card]));
  const stands = (line: Line, edge: RequirementEdge) => {
    const [from, to] = [line.from, line.to].map((key) => cardOf.get(key)).map((card) => (card === undefined ? [] : standsFor(card)));
    return (from?.includes(edge.node) === true && to?.includes(edge.requires) === true) || (from?.includes(edge.requires) === true && to?.includes(edge.node) === true);
  };
  const toned = (line: Line): LineMark => (drawn.removedLines?.has(line.id) === true ? "remove" : edges.added.some((edge) => stands(line, edge)) ? "add" : edges.removed.some((edge) => stands(line, edge)) ? "remove" : "lit");
  const lines = Object.fromEntries(model.lines.filter((line) => shown[line.from] !== undefined || shown[line.to] !== undefined || toned(line) !== "lit").map((line) => [line.id, toned(line)]));
  const present = new Set(model.cards.flatMap(standsFor));
  const outside = Object.keys(marks).filter((key) => !present.has(key)).sort();
  return { title, marks: shown, lines, dim: false, outside };
}
