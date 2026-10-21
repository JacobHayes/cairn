// What a graph canvas draws, for a journey and for a route alike (C1 to C7, C14, C15): the view's
// level read from the derivation the worker holds, turned into the canvas model by the side that
// owns the cards' looks, laid out in the layout worker, and the overlay over it: the proposal
// layer's marks, else the selected node's trace. A node selected is traced (a card standing for
// hidden nodes traces what they trace), Reveal draws the traced nodes the view hides, and
// Frontier and Depth's Fade fade the rest. The two canvases (JourneyCanvas.tsx, RouteCanvas.tsx)
// supply their side, their actions and their chrome; nothing of this is written twice.
import { useCallback, useEffect, useMemo, useState } from "react";
import { useLocation } from "react-router";

import { errorWords, valueOf } from "../data/keyed-read.ts";
import { KINDS, type GraphNode, type Ready } from "../detail/model.ts";
import { journeyLayoutView, type LevelRequest } from "../journeys/requests.ts";
import { withRemoved, type Composed, type Layer } from "../proposals/layer.ts";
import { itemNode } from "../proposals/model.ts";
import type { Reveal } from "./GraphCanvas.tsx";
import { useLaidOut, useLevel, useProjected, type Held } from "./hooks.ts";
import { ancestorsOf } from "./ladder.ts";
import { STRENGTH, type CanvasModel, type Level, type Line } from "./model.ts";
import { marksOverlay, standsFor, traceOverlay } from "./overlay.ts";
import type { GraphView } from "./settings.ts";
import { useViewLevel } from "./viewLevel.ts";

/**
 * One graph the canvas draws a level of: the derivation it is read from, its nodes, and how its
 * level becomes cards and lines (the side's own looks). `model` is none while the side is not
 * ready to draw (a journey waits for the nodes that are the viewer's).
 */
export interface Side {
  held: Held;
  nodes: readonly GraphNode[];
  model: ((level: Level) => CanvasModel) | undefined;
}

/**
 * What Reveal shows beside the view (5.7): the level with every kind and class and no filter or
 * search, less what the view does not draw (`drawn`, the view's own level under the same
 * collapse) and the trace does not touch. The traced nodes the view hides are drawn, ghosted
 * where not relevant.
 */
export function onlyTraced(model: CanvasModel, traced: ReadonlySet<string>, drawn: ReadonlySet<string>): CanvasModel {
  const out = new Set(model.cards.filter((card) => !drawn.has(card.key) && !standsFor(card).some((key) => traced.has(key))).map((card) => card.key));
  // A line to a card left out runs to the nearest card kept above it, as the view would have drawn it (C2).
  const byKey = new Map(model.cards.map((card) => [card.key, card]));
  const standIn = (key: string): string | undefined => {
    let at = byKey.get(key);
    while (at !== undefined && out.has(at.key)) {
      at = at.parent === undefined ? undefined : byKey.get(at.parent);
    }
    return at?.key;
  };
  const lines = new Map<string, Line>();
  for (const line of model.lines) {
    const [from, to] = [standIn(line.from), standIn(line.to)];
    if (from === undefined || to === undefined || from === to) {
      continue;
    }
    const [id, kept] = [`${from}->${to}`, lines.get(`${from}->${to}`)];
    // A line with no state (a route's) stays without after the merge; one with state is satisfied only when both are.
    const satisfied = kept === undefined ? line.satisfied : kept.satisfied === undefined && line.satisfied === undefined ? undefined : kept.satisfied === true && line.satisfied === true;
    lines.set(
      id,
      kept === undefined
        ? { ...line, id, from, to }
        : {
            ...kept,
            kind: STRENGTH[line.kind] > STRENGTH[kept.kind] ? line.kind : kept.kind,
            count: kept.count + line.count,
            implicit: kept.implicit && line.implicit,
            gates: kept.gates || line.gates,
            ...(satisfied === undefined ? {} : { satisfied }),
          },
    );
  }
  return { cards: model.cards.filter((card) => !out.has(card.key)), lines: [...lines.values()] };
}

/** What the trace reaches that has no card of its own (outside the step, folded into a container, or left out). */
export function outsideOf(trace: { upstream: string[]; downstream: string[] } | undefined, model: CanvasModel): number {
  const drawnAs = new Set(model.cards.map((card) => card.key));
  return trace === undefined ? 0 : [...trace.upstream, ...trace.downstream].filter((key) => !drawnAs.has(key)).length;
}

/**
 * The selected node's trace. A card standing for hidden nodes (a collapsed stage) traces what they
 * trace (C2, C7), so the line that leaves it is followed; what the card stands for is learned from
 * the drawn canvas, once it is.
 */
function useSelectionTrace(held: Held, selected: string | undefined) {
  const own = useProjected(held, selected === undefined ? undefined : { projection: "trace", key: selected });
  const [stands, setStands] = useState<{ node: string | undefined; keys: string[] }>({ node: undefined, keys: [] });
  const standing = stands.node === selected ? stands.keys : [];
  // One walk from the card's node and every node it stands for; until it arrives, or if it fails, the node's own trace stands.
  const joinedRead = useProjected(held, selected === undefined || standing.length === 0 ? undefined : { projection: "trace", key: selected, standing });
  const joined = joinedRead.status === "current" ? joinedRead.value : undefined;
  const learn = useCallback((rolledUp: string[]) => {
    setStands((was) => (was.node === selected && was.keys.join() === rolledUp.join() ? was : { node: selected, keys: rolledUp }));
  }, [selected]);
  return { trace: joined ?? valueOf(own), learn };
}

export interface CanvasGraphArgs {
  view: GraphView;
  /** The graph drawn: the journey, or the route's shown graph. */
  side: Side;
  /** Under a proposal's layer, the graph as it is, at the same level, which supplies what the proposal removes, put back where it was. */
  was: Side | undefined;
  /** The before side is still being derived: the canvas waits for it, as what it draws changes then. */
  wait?: boolean;
  layer: Layer | undefined;
  /** The journey, for the stages that are current and the nodes Frontier lights; none for a route, which has neither. */
  ready: Ready | undefined;
  /** The one node that is selected and traced. */
  selected: string | undefined;
  /** The node the address names, which the level always draws (C7). */
  address: string | undefined;
  /** What the layout is of: another domain's positions never hint this one's. */
  domain: string;
  /** A node's title, for the trace's legend. */
  title: (key: string) => string;
}

/** The view's level (C2), and while Reveal is on, the whole graph's beside it under the same collapse, less what neither draws nor the trace reaches. */
function useComposed(args: { side: Side; was: Side | undefined; wait: boolean; layer: Layer | undefined; level: Level | undefined; collapsed: string[]; revealed: ReadonlySet<string> | undefined; wasLevel: Level | undefined; wasFailed: boolean }) {
  const { side, was, wait, layer, level, collapsed, revealed, wasLevel, wasFailed } = args;
  const everything = useMemo<LevelRequest | undefined>(() => (revealed === undefined ? undefined : { projection: "level", shown: KINDS, ...(collapsed.length === 0 ? {} : { collapsed }) }), [revealed, collapsed]);
  const wholeRead = useProjected(side.held, everything);
  const whole = valueOf(wholeRead);
  const build = side.model;
  const wasBuild = was?.model;
  const composed = useMemo<Composed | undefined>(() => {
    const shown = revealed === undefined ? level : whole;
    if (level === undefined || shown === undefined || build === undefined || wait || (was !== undefined && wasLevel === undefined && !wasFailed)) {
      return undefined;
    }
    const drawn = build(shown);
    const kept = revealed === undefined ? drawn : onlyTraced(drawn, revealed, new Set(level.nodes.map((node) => node.key)));
    if (layer?.model === undefined || wasBuild === undefined || wasLevel === undefined) {
      return { model: kept, removedLines: new Set(), shrunk: new Map() };
    }
    return withRemoved(kept, wasBuild(wasLevel), layer.removed, layer.model.diff.edgesRemoved);
  }, [build, level, whole, revealed, was, wait, wasBuild, wasLevel, wasFailed, layer]);
  return { composed, error: errorWords(wholeRead) };
}

/** What the canvas draws: the view's level laid out, the selected node's trace over it, what the highlights light, and what Reveal asks for. */
export function useCanvasGraph({ view, side, was, wait = false, layer, ready, selected, address, domain, title }: CanvasGraphArgs) {
  const { nodes } = side;
  // The trace and the diff never show at once: under a proposal's layer a pick is for the rail.
  const { trace, learn } = useSelectionTrace(side.held, layer === undefined ? selected : undefined);
  // A node the proposal removes has no card in the graph after; its address is drawn from the level before, so the level after is asked for no node.
  const asked = layer !== undefined && address !== undefined && !nodes.some((node) => node.key === address) ? undefined : address;
  // Reveal (5.7) is a moment, not an address: it ends when something else is selected.
  const [revealedFor, setRevealedFor] = useState<{ node: string | undefined; linked: number } | undefined>(undefined);
  const traced = useMemo(() => (trace === undefined ? undefined : new Set([trace.node, ...trace.upstream, ...trace.downstream])), [trace]);
  const revealed = revealedFor !== undefined && revealedFor.node === selected ? revealedFor : undefined;
  const traceOnly = revealed === undefined ? undefined : traced;
  const open = useMemo(() => (traceOnly === undefined ? [] : [...traceOnly].flatMap((key) => ancestorsOf(nodes, key))), [traceOnly, nodes]);
  const { depth, collapsed, request, level, error: levelError, faded, base: lit, badges } = useViewLevel({
    screen: "graph",
    state: view.state,
    nodes,
    selected: asked,
    ready,
    held: side.held,
    folds: view,
    unfold: open,
    alsoDrawn: traceOnly,
  });
  const removedAt = asked === address ? undefined : address;
  const wasRead = useLevel(was?.held, was?.nodes ?? NO_NODES, was === undefined ? undefined : request, removedAt);
  const { composed, error: wholeError } = useComposed({ side, was, wait, layer, level, collapsed, revealed: traceOnly, wasLevel: wasRead.level, wasFailed: wasRead.error !== undefined });
  const model = composed?.model;
  // Frontier, Fade's dimming and the viewer's expands and collapses keep the layout's positions; the step drawn, the Filter and the search lay it out afresh.
  const laidRead = useLaidOut(domain, journeyLayoutView(view.state, depth), model);
  const laidOut = valueOf(laidRead);
  const rolledUp = laidOut?.model.cards.find((card) => card.key === selected)?.rolledUp;
  useEffect(() => {
    if (rolledUp !== undefined) {
      learn(rolledUp);
    }
  }, [rolledUp, learn]);
  const overlay = useMemo(() => {
    if (laidOut === undefined) {
      return undefined;
    }
    if (layer?.model !== undefined) {
      const { diff, marks } = layer.model;
      return marksOverlay("What the proposal changes", marks, laidOut.model, { added: diff.edgesAdded, removed: diff.edgesRemoved }, composed);
    }
    return selected !== undefined && trace !== undefined ? traceOverlay(trace, laidOut.model, title(trace.node)) : undefined;
  }, [selected, trace, laidOut, layer, composed, title]);
  return {
    depth,
    trace,
    faded,
    laidOut,
    overlay,
    lit,
    badges,
    layer,
    revealed: revealed?.linked,
    error: levelError ?? wholeError ?? errorWords(laidRead),
    reveal: (linked: number) => { setRevealedFor({ node: selected, linked }); },
    hide: () => { setRevealedFor(undefined); },
  };
}

const NO_NODES: readonly GraphNode[] = [];

/**
 * The nodes a first fit frames under the layer, in the order the rail lists them: those waiting on a choice, then the other nodes the proposal marks, then the ends of the requirements it changes. Empty while the destination after is still being derived (a preview that finished without one, an invalid proposal's, is not waited for): the canvas waits for it, as what it draws changes then.
 */
export function changedKeys(layer: Layer): string[] | undefined {
  const model = layer.model;
  if (model === undefined) {
    return undefined;
  }
  if (!model.known && !model.previewed && layer.mode === "reviewing" && model.previewError === undefined) {
    return [];
  }
  // The rail's own order: the items waiting on a choice as its fold lists them, then its changes.
  const waiting = (model.draft.items ?? []).flatMap((item, index) => (model.unresolved.has(index) ? [itemNode(item)] : [])).filter((key): key is string => key !== undefined);
  const keys = [...waiting, ...model.entries.map((entry) => entry.key), ...[...model.diff.edgesAdded, ...model.diff.edgesRemoved].flatMap((edge) => [edge.node, edge.requires])];
  return keys.length === 0 ? undefined : [...new Set(keys)];
}

/**
 * Where the canvas is asked to go: what a navigation asked for (its `reveal` state), else, while
 * the location is still the one the canvas mounted at, the node its address names, centred once. Any
 * navigation changes the location, so an opening request is gone for good and never comes back to
 * centre the first node after a later reveal.
 */
export function useRevealed(selected: string | undefined): Reveal | undefined {
  const location = useLocation();
  const [opened] = useState(() => ({ key: location.key, node: selected }));
  const asked = (location.state as { reveal?: { key: string; how: Reveal["how"] } } | null)?.reveal;
  return useMemo<Reveal | undefined>(() => {
    if (asked !== undefined) {
      return { key: asked.key, how: asked.how, id: location.key };
    }
    return opened.key === location.key && opened.node !== undefined ? { key: opened.node, how: "centre", id: location.key } : undefined;
  }, [asked, opened, location.key]);
}
