// C14: a proposal as a diff over the canvas, filling the frame's workspace like any graph. The
// graph after is drawn with every node the proposal removes put back where it was (struck and
// ghosted), all kinds shown, laid out as any canvas is (C15); the overlay (5.2's CanvasOverlay)
// tags each card with the word for what is added, changed, removed, in conflict, orphaned, or
// left out of a saved route, and fades the rest when a filter is on. It carries no state: what
// can be acted on after is the frontier in the proposal card. Opening a card picks its node
// for the inspector's item editor. The trace is off during review: the diff and a trace never
// show at once.
import { useEffect, useMemo, useState } from "react";

import type { Graph } from "../authoring/graph.ts";
import { GraphCanvas } from "../canvas/GraphCanvas.tsx";
import { useLaidOut } from "../canvas/hooks.ts";
import { KINDS, cardsOf, linesOf, type CanvasModel, type Level } from "../canvas/model.ts";
import type { CardActions } from "../canvas/NodeCard.tsx";
import { marksOverlay } from "../canvas/overlay.ts";
import type { Deployment, Deriver } from "../data/host.ts";
import { errorWords, PROJECTION, valueOf, type ReadSource } from "../data/keyed-read.ts";
import { useKeyedRead, useSession } from "../data/react.ts";
import type { DiffMark, GraphDiff } from "./model.ts";
import { NODE_GONE } from "../ui/gone.ts";

/** What a proposal canvas's level is derived from. */
interface LevelArgs {
  deriver: Deriver;
  graph: Graph;
  deployment: Deployment;
  today: string;
}

const LEVEL: ReadSource<LevelArgs, { level: Level; graph: Graph }> = {
  policy: PROJECTION,
  read: async ({ deriver, graph, deployment, today }) => ({ level: await deriver.routeLevel({ graph, deployment, today, query: { shown: KINDS } }), graph }),
};

/** C2: the level of `graph` with every kind shown, or why there is none; `tried` is which of the canvas's graphs this is. */
function useLevel(domain: string, tried: number, graph: Graph | undefined, deployment: Deployment, today: string): { level: Level; graph: Graph } | { error: string } | undefined {
  const { deriver } = useSession();
  const { view } = useKeyedRead(LEVEL, graph === undefined ? undefined : { identity: [domain, tried], freshness: [graph, deployment.revision, today], args: { deriver, graph, deployment, today } });
  if (view.status === "failed" || view.status === "behind") {
    return { error: view.error.message };
  }
  return valueOf(view);
}

export interface ProposalCanvasProps {
  /** Its key, for the layout cache. */
  domain: string;
  /** What is drawn: the graph after with the removed nodes put back; the graph after alone when that does not hold together. */
  graphs: Graph[];
  marks: Record<string, DiffMark>;
  /** The diff's requirements, which the canvas draws as added or removed lines. */
  edges: Pick<GraphDiff, "edgesAdded" | "edgesRemoved">;
  /** A filter is on: the cards it does not keep fade. */
  dim: boolean;
  deployment: Deployment;
  today: string;
  selected: string | undefined;
  onPick: (key: string) => void;
}

export function ProposalCanvas({ domain, graphs, marks, edges, dim, deployment, today, selected, onPick }: ProposalCanvasProps) {
  // The first graph that the engine draws: a union can break an invariant a graph after holds.
  const [tried, setTried] = useState(0);
  useEffect(() => {
    setTried(0);
  }, [graphs]);
  const graph = graphs[tried];
  const answer = useLevel(domain, tried, graph, deployment, today);
  useEffect(() => {
    if (answer !== undefined && "error" in answer && tried + 1 < graphs.length) {
      setTried(tried + 1);
    }
  }, [answer, tried, graphs.length]);
  const model = useMemo<CanvasModel | undefined>(() => {
    if (answer === undefined || "error" in answer) {
      return undefined;
    }
    const nodes = answer.graph.nodes ?? [];
    return { cards: cardsOf(answer.level, nodes), lines: linesOf(answer.level, nodes) };
  }, [answer]);
  const laid = useLaidOut(`proposal:${domain}`, "diff", model);
  const laidOut = valueOf(laid);
  const error = errorWords(laid);
  const overlay = useMemo(() => (laidOut === undefined ? undefined : marksOverlay("What the proposal changes", marks, laidOut.model, dim, { added: edges.edgesAdded, removed: edges.edgesRemoved })), [laidOut, marks, edges, dim]);
  // Opens on the changes (a changed requirement frames both its ends), so a small edit to a big journey is readable; Fit still fits the whole graph.
  const changed = useMemo(() => [...Object.keys(marks), ...[...edges.edgesAdded, ...edges.edgesRemoved].flatMap((edge) => [edge.node, edge.requires])], [marks, edges]);
  const actions = useMemo<CardActions>(
    () => ({
      open: onPick,
      expand: undefined,
      trace: undefined,
      title: (key) => (graph?.nodes ?? []).find((node) => node.key === key)?.title ?? NODE_GONE,
      menu: undefined,
    }),
    [onPick, graph],
  );
  if (answer !== undefined && "error" in answer && tried + 1 >= graphs.length) {
    return <p className="callout callout-bad">The diff could not be drawn: {answer.error}</p>;
  }
  if (error !== undefined) {
    return <p className="callout callout-bad">The diff could not be laid out: {error}</p>;
  }
  if (laidOut === undefined || overlay === undefined) {
    return <p className="muted small">Laying out the diff...</p>;
  }
  return (
    <GraphCanvas model={laidOut.model} layout={laidOut.layout} overlay={overlay} lens={undefined} selected={selected} actions={actions} firstFit={changed} viewKey={`${laidOut.view}:${String(laidOut.model.cards.length)}`} label="The proposal's diff" />
  );
}
