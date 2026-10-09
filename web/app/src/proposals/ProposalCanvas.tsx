// C14: a proposal as a diff over the canvas. The graph after is drawn with every node the
// proposal removes put back where it was, all kinds shown, laid out as any canvas is (C15);
// the overlay (5.2's CanvasOverlay) marks what is added, changed, removed, in conflict,
// orphaned, or left out of a saved route. It carries no state: what can be acted on after is
// the frontier beside it. Opening a card picks its node in the lists.
import { useEffect, useMemo, useState } from "react";

import type { Graph } from "../authoring/graph.ts";
import { GraphCanvas } from "../canvas/GraphCanvas.tsx";
import { useLaidOut } from "../canvas/hooks.ts";
import { KINDS, cardsOf, linesOf, type CanvasModel, type Level } from "../canvas/model.ts";
import type { CardActions } from "../canvas/NodeCard.tsx";
import { marksOverlay } from "../canvas/overlay.ts";
import type { Deployment } from "../data/host.ts";
import { useSession } from "../data/react.ts";
import { DIFF_LABELS, type DiffMark } from "./model.ts";

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
  deployment: Deployment;
  today: string;
  selected: string | undefined;
  onPick: (key: string) => void;
}

export function ProposalCanvas({ domain, graphs, marks, deployment, today, selected, onPick }: ProposalCanvasProps) {
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
  const overlay = useMemo(() => (laidOut === undefined ? undefined : marksOverlay("What the proposal changes", marks, laidOut.model)), [laidOut, marks]);
  const actions = useMemo<CardActions>(
    () => ({
      open: onPick,
      drill: undefined,
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
    <div className="stack" data-testid="proposal-canvas">
      <div className="row muted small" data-testid="diff-legend">
        {Object.values(DIFF_LABELS).map((label) => (
          <span key={label} className="proposal-legend">{label}</span>
        ))}
      </div>
      <div className="proposal-canvas">
        <GraphCanvas model={laidOut.model} placement={laidOut.placement} overlay={overlay} heat={false} selected={selected} actions={actions} viewKey={`${laidOut.view}:${String(laidOut.model.cards.length)}`} label="The proposal's diff" inScroller />
      </div>
      {overlay.outside.length === 0 ? null : <span className="muted small">Also changed, not drawn: {overlay.outside.join(", ")}</span>}
    </div>
  );
}
