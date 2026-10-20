// C19: the incoming root and its wiring drawn on the graph at their proposed place, marked as the
// proposal layer marks an addition (the fill, the solid edge and the word ADDED), so what the
// stepper shows is what a proposal's review will. A segment only adds, so this draws the graph
// after alone: a trimmed canvas that reads the level and lays it out, with no state and no actions.
import { useMemo } from "react";

import type { Graph } from "../authoring/graph.ts";
import { KINDS } from "../detail/model.ts";
import { GraphCanvas } from "../canvas/GraphCanvas.tsx";
import { useLaidOut } from "../canvas/hooks.ts";
import { cardsOf, linesOf, type CanvasModel, type Level } from "../canvas/model.ts";
import type { CardActions } from "../canvas/NodeCard.tsx";
import { marksOverlay } from "../canvas/overlay.ts";
import type { Deployment, Deriver } from "../data/host.ts";
import { errorWords, PROJECTION, valueOf, type ReadSource } from "../data/keyed-read.ts";
import { useDeployment, useKeyedRead, useSession } from "../data/react.ts";
import { diffMarks, graphDiff } from "../proposals/model.ts";
import { NODE_GONE } from "../ui/gone.ts";
import { noteOrigins } from "./model.ts";
import type { InsertPreview } from "./Stepper.tsx";

interface LevelArgs {
  deriver: Deriver;
  graph: Graph;
  deployment: Deployment;
  today: string;
}

const LEVEL: ReadSource<LevelArgs, Level> = {
  policy: PROJECTION,
  read: async ({ deriver, graph, deployment, today }) => deriver.routeLevel({ graph, deployment, today, query: { shown: KINDS } }),
};

export function InsertionCanvas({ preview, domain, today }: { preview: InsertPreview; domain: string; today: string }) {
  const deployment = useDeployment();
  const { deriver } = useSession();
  const diff = useMemo(() => graphDiff(preview.before, preview.after), [preview]);
  const marks = useMemo(() => noteOrigins(diffMarks(diff, [], new Set()), preview.members), [diff, preview]);
  const { view } = useKeyedRead(LEVEL, deployment === undefined ? undefined : { of: [`insert:${domain}`], freshness: [preview.after, deployment.revision, today], args: { deriver, graph: preview.after, deployment, today } });
  const level = valueOf(view);
  const model = useMemo<CanvasModel | undefined>(() => {
    const nodes = preview.after.nodes ?? [];
    return level === undefined ? undefined : { cards: cardsOf(level, nodes), lines: linesOf(level, nodes) };
  }, [level, preview.after]);
  const laid = useLaidOut(`insert:${domain}`, "diff", model);
  const laidOut = valueOf(laid);
  const overlay = useMemo(() => (laidOut === undefined ? undefined : marksOverlay("What the segment adds", marks, laidOut.model, { added: diff.edgesAdded, removed: diff.edgesRemoved })), [laidOut, marks, diff]);
  const actions = useMemo<CardActions>(
    () => ({ open: undefined, expand: undefined, trace: undefined, title: (key) => (preview.after.nodes ?? []).find((node) => node.key === key)?.title ?? NODE_GONE, menu: undefined }),
    [preview.after],
  );
  const changed = useMemo(() => [...Object.keys(marks), ...[...diff.edgesAdded].flatMap((edge) => [edge.node, edge.requires])], [marks, diff]);
  const error = errorWords(view) ?? errorWords(laid);
  if (error !== undefined) {
    return <p className="callout callout-bad">The preview could not be drawn: {error}</p>;
  }
  if (laidOut === undefined || overlay === undefined) {
    return <p className="muted small">Laying out the canvas...</p>;
  }
  return <GraphCanvas model={laidOut.model} layout={laidOut.layout} overlay={overlay} lens={undefined} selected={undefined} actions={actions} firstFit={changed} viewKey={`${laidOut.view}:${String(laidOut.model.cards.length)}`} label="The segment's preview" />;
}
