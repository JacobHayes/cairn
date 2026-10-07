// The canvas itself (C1, C3): React Flow for pan, zoom, nested containers, custom cards, and
// dotted implicit edges (ARCHITECTURE, Web UI: Canvas), over cards and lines already laid out.
// Nothing is dragged (layout is automatic, C15; pinned positions are Later), nothing animates
// (C6), and the view fits the whole graph when what it shows changes, not on every revision,
// so a live edit never jumps the viewport.
import "@xyflow/react/dist/base.css";
import "./canvas.css";

import { Background, Controls, MarkerType, ReactFlow, ReactFlowProvider } from "@xyflow/react";
import { useMemo } from "react";

import { cardHeight, cardLines } from "./cards.ts";
import { EdgeLine, type LineEdge } from "./EdgeLine.tsx";
import type { Placement } from "./layout.ts";
import type { CanvasModel } from "./model.ts";
import { CardActionsContext, NodeCard, type CardActions, type CardNode } from "./NodeCard.tsx";
import type { CanvasOverlay } from "./overlay.ts";

const nodeTypes = { card: NodeCard };
const edgeTypes = { line: EdgeLine };

/** C3: the zoom range, wide enough to see a whole journey at its limits. */
const ZOOM_MIN = 0.05;
const ZOOM_MAX = 2;

export interface GraphCanvasProps {
  model: CanvasModel;
  placement: Placement;
  overlay: CanvasOverlay | undefined;
  heat: boolean;
  selected: string | undefined;
  actions: CardActions;
  /** What the canvas shows; the view fits the graph again when it changes. */
  viewKey: string;
  label: string;
}

function nodesOf({ model, placement, overlay, heat, selected }: GraphCanvasProps): CardNode[] {
  const containers = new Set(model.cards.flatMap((card) => (card.parent === undefined ? [] : [card.parent])));
  return model.cards.flatMap((card) => {
    const placed = placement[card.key];
    if (placed === undefined) {
      return [];
    }
    const node: CardNode = {
      id: card.key,
      type: "card",
      position: { x: placed.x, y: placed.y },
      width: placed.width,
      height: placed.height,
      // A card is drawn at the size it was laid out at, so it is measured already. Without
      // this, React Flow takes each revision's new node objects for unmeasured ones: it drops
      // their handle bounds and observes every card afresh from inside its resize callback,
      // which Chromium reports as a ResizeObserver loop error on every edit. A card whose
      // size does change stays observed and is measured again.
      measured: { width: placed.width, height: placed.height },
      data: {
        card,
        overlay,
        heat,
        selected: card.key === selected,
        header: containers.has(card.key) ? cardHeight(cardLines(card)) : undefined,
        place: { x: Math.round(placed.x), y: Math.round(placed.y) },
      },
      draggable: false,
      selectable: false,
      connectable: false,
    };
    if (card.parent !== undefined) {
      node.parentId = card.parent;
    }
    return [node];
  });
}

function edgesOf({ model, overlay }: GraphCanvasProps): LineEdge[] {
  return model.lines.map((line) => ({
    id: line.id,
    source: line.from,
    target: line.to,
    type: "line",
    data: { line, overlay },
    markerEnd: { type: MarkerType.ArrowClosed, width: 16, height: 16 },
    selectable: false,
    focusable: false,
  }));
}

function Flow(props: GraphCanvasProps) {
  const nodes = useMemo(() => nodesOf(props), [props]);
  const edges = useMemo(() => edgesOf(props), [props]);
  return (
    <CardActionsContext value={props.actions}>
      <ReactFlow
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        edgeTypes={edgeTypes}
        nodesDraggable={false}
        nodesConnectable={false}
        elementsSelectable={false}
        onNodeClick={(_, node) => {
          props.actions.open?.(node.id);
        }}
        zIndexMode="auto"
        minZoom={ZOOM_MIN}
        maxZoom={ZOOM_MAX}
        fitView
        colorMode="system"
        aria-label={props.label}
      >
        <Background gap={24} size={1} />
        <Controls showInteractive={false} />
      </ReactFlow>
    </CardActionsContext>
  );
}

/** A graph's canvas: laid-out cards and lines, with pan and zoom. */
export function GraphCanvas(props: GraphCanvasProps) {
  return (
    <div className="canvas" data-testid="canvas" data-view={props.viewKey}>
      {/* A new view mounts a new flow, which fits the whole graph (C3); a revision keeps the viewport. */}
      <ReactFlowProvider key={props.viewKey}>
        <Flow {...props} />
      </ReactFlowProvider>
    </div>
  );
}
