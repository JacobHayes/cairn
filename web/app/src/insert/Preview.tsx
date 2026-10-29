// C19: the incoming root and its wiring drawn on the graph at their proposed place, marked as the
// proposal layer marks an addition (the fill, the solid edge and the word ADDED), so what the
// stepper shows is what a proposal's review will. A route only adds, so this draws the graph
// after alone: a trimmed canvas that reads the level and lays it out, with no state and no actions.
import { useMemo } from "react";

import type { Graph } from "../authoring/graph.ts";
import type { LevelRequest } from "../journeys/requests.ts";
import { derivedSource } from "../screens/route-reads.ts";
import { KINDS, type GraphNode } from "../detail/model.ts";
import { GraphCanvas } from "../canvas/GraphCanvas.tsx";
import { useLaidOut, useLevel } from "../canvas/hooks.ts";
import { cardsOf, linesOf, type CanvasModel } from "../canvas/model.ts";
import type { CardActions } from "../canvas/NodeCard.tsx";
import { marksOverlay } from "../canvas/overlay.ts";
import { errorWords, valueOf } from "../data/keyed-read.ts";
import { useDeployment, useKeyedRead, useSession } from "../data/react.ts";
import { diffMarks, graphDiff } from "../proposals/model.ts";
import { NODE_GONE } from "../ui/gone.ts";
import { noteOrigins } from "./model.ts";
import type { InsertPreview } from "./Stepper.tsx";

const NO_NODES: GraphNode[] = [];

/** Every kind at the top level, nothing collapsed: the whole graph. */
const EVERYTHING: LevelRequest = { projection: "level", shown: KINDS };

/** The graph after, derived and held beside the page's own. */
const PREVIEW_DERIVED = derivedSource<{ graph: Graph }>();

export function InsertionCanvas({ preview, domain }: { preview: InsertPreview; domain: string }) {
  const deployment = useDeployment();
  const { deriver } = useSession();
  const diff = useMemo(() => graphDiff(preview.before, preview.after), [preview]);
  const marks = useMemo(() => noteOrigins(diffMarks(diff, []), preview.members), [diff, preview]);
  const subject = useMemo(() => ({ graph: preview.after }), [preview.after]);
  const reads = `insert:${domain}`;
  const derivedRead = useKeyedRead(PREVIEW_DERIVED, deployment === undefined ? undefined : { of: [reads], freshness: [preview.after, deployment.revision], args: { deriver, subject, deployment, reads } }).view;
  const derived = valueOf(derivedRead);
  const nodes = preview.after.nodes ?? NO_NODES;
  const { level, error: levelError } = useLevel(derived, nodes, derived === undefined ? undefined : EVERYTHING);
  const model = useMemo<CanvasModel | undefined>(() => (level === undefined ? undefined : { cards: cardsOf(level, nodes), lines: linesOf(level, nodes) }), [level, nodes]);
  const laid = useLaidOut(`insert:${domain}`, "diff", model);
  const laidOut = valueOf(laid);
  const overlay = useMemo(() => (laidOut === undefined ? undefined : marksOverlay("What the route adds", marks, laidOut.model, { added: diff.edgesAdded, removed: diff.edgesRemoved })), [laidOut, marks, diff]);
  const actions = useMemo<CardActions>(
    () => ({ open: undefined, expand: undefined, trace: undefined, title: (key) => (preview.after.nodes ?? []).find((node) => node.key === key)?.title ?? NODE_GONE, menu: undefined }),
    [preview.after],
  );
  const changed = useMemo(() => [...Object.keys(marks), ...[...diff.edgesAdded].flatMap((edge) => [edge.node, edge.requires])], [marks, diff]);
  const error = errorWords(derivedRead) ?? levelError ?? errorWords(laid);
  if (error !== undefined) {
    return <p className="callout callout-bad">The preview could not be drawn: {error}</p>;
  }
  if (laidOut === undefined || overlay === undefined) {
    return <p className="muted small">Laying out the canvas...</p>;
  }
  return <GraphCanvas model={laidOut.model} layout={laidOut.layout} overlay={overlay} lens={undefined} selected={undefined} actions={actions} firstFit={changed} viewKey={`${laidOut.view}:${String(laidOut.model.cards.length)}`} label="The route's preview" />;
}
