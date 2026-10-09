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
import type { Deployment } from "../data/host.ts";
import { useSession } from "../data/react.ts";
import type { DiffMark } from "./model.ts";

/** C2: the level of `graph` with every kind shown, or why there is none. */
function useLevel(graph: Graph | undefined, deployment: Deployment, today: string): { level: Level; graph: Graph } | { error: string } | undefined {
  const { deriver } = useSession();
  const [answer, setAnswer] = useState<{ level: Level; graph: Graph } | { error: string } | undefined>();
  useEffect(() => {
    if (graph === undefined) {
      return;
    }
    let live = true;
    deriver.routeLevel({ graph, deployment, today, shown: KINDS }).then(
      (level) => {
        if (live) {
          setAnswer({ level, graph });
        }
      },
      (thrown: unknown) => {
        if (live) {
          setAnswer({ error: thrown instanceof Error ? thrown.message : String(thrown) });
        }
      },
    );
    return () => {
      live = false;
    };
  }, [deriver, graph, deployment, today]);
  return answer;
}

export interface ProposalCanvasProps {
  /** Its key, for the layout cache. */
  domain: string;
  /** What is drawn: the graph after with the removed nodes put back; the graph after alone when that does not hold together. */
  graphs: Graph[];
  marks: Record<string, DiffMark>;
  /** A filter is on: the cards it does not keep fade. */
  dim: boolean;
  deployment: Deployment;
  today: string;
  selected: string | undefined;
  onPick: (key: string) => void;
}

export function ProposalCanvas({ domain, graphs, marks, dim, deployment, today, selected, onPick }: ProposalCanvasProps) {
  // The first graph that the engine draws: a union can break an invariant a graph after holds.
  const [tried, setTried] = useState(0);
  useEffect(() => {
    setTried(0);
  }, [graphs]);
  const graph = graphs[tried];
  const answer = useLevel(graph, deployment, today);
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
  const { laidOut, error } = useLaidOut(`proposal:${domain}`, "diff", model);
  const overlay = useMemo(() => (laidOut === undefined ? undefined : marksOverlay("What the proposal changes", marks, laidOut.model, dim)), [laidOut, marks, dim]);
  // The view opens on the changes (all of them when they fit at a readable zoom, else the first one), not fitted to a journey whose cards are too small to read.
  const focus = useMemo(() => {
    const changed = Object.keys(marks);
    return changed.length === 0 ? undefined : [changed, changed.slice(0, 1)];
  }, [marks]);
  const actions = useMemo<CardActions>(
    () => ({
      open: onPick,
      drill: undefined,
      expand: undefined,
      trace: undefined,
      title: (key) => (graph?.nodes ?? []).find((node) => node.key === key)?.title ?? key,
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
    <GraphCanvas model={laidOut.model} layout={laidOut.layout} overlay={overlay} lens={undefined} selected={selected} actions={actions} focus={focus} viewKey={`${laidOut.view}:${String(laidOut.model.cards.length)}`} label="The proposal's diff" title="The proposal's diff" />
  );
}
