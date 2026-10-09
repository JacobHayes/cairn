// The canvas itself (C1, C3): React Flow for pan, zoom, nested containers, custom cards, and
// dotted implicit edges (ARCHITECTURE, Web UI: Canvas), over cards and lines already laid out.
// Nothing is dragged (layout is automatic, C15; pinned positions are Later), nothing animates
// (C6), and the view fits the whole graph when what it shows changes, not on every revision,
// so a live edit never jumps the viewport.
import "@xyflow/react/dist/base.css";
import "./canvas.css";

import { Controls, MarkerType, ReactFlow, ReactFlowProvider, useReactFlow } from "@xyflow/react";
import { useEffect, useMemo } from "react";

import { PHONE_WIDTH, SHEET, useFrameState } from "../shell/frame.tsx";
import { useResolvedTheme } from "../ui/theme.ts";
import { cardHeight, cardLines } from "./cards.ts";
import { EdgeLine, type LineEdge } from "./EdgeLine.tsx";
import type { Placement } from "./layout.ts";
import { MapFrame } from "./MapFrame.tsx";
import type { CanvasModel } from "./model.ts";
import { CardActionsContext, NodeCard, type CardActions, type CardNode } from "./NodeCard.tsx";
import type { CanvasOverlay } from "./overlay.ts";

const nodeTypes = { card: NodeCard };
const edgeTypes = { line: EdgeLine };

/** C3: the zoom range, wide enough to see a whole journey at its limits. */
const ZOOM_MIN = 0.05;
const ZOOM_MAX = 2;
/** A small graph fits at 100% rather than blown up to fill the region. */
const FIT = { maxZoom: 1, padding: 0.1 };

export interface GraphCanvasProps {
  model: CanvasModel;
  placement: Placement;
  overlay: CanvasOverlay | undefined;
  heat: boolean;
  selected: string | undefined;
  actions: CardActions;
  /** What the canvas shows; the view fits the graph again when it changes. */
  viewKey: string;
  /** The accessible name of the canvas. */
  label: string;
  /** What the map is of, as its full-screen bar shows it (the journey's name, not the label). */
  title: string;
  /** The canvas sits inside a page that scrolls: the wheel scrolls the page, and drag and pinch still pan and zoom. */
  inScroller?: boolean;
}

/** `inert`: the phone's preview, which takes no gesture and opens nothing (MapFrame). */
interface FlowProps extends GraphCanvasProps {
  inert: boolean;
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

/**
 * On a tablet, and in a phone's full-screen map, the inspector is a sheet over the canvas's lower
 * part: a node picked is centred in the band the sheet leaves, so the selection is never under it.
 */
function useCenterAboveSheet(selected: string | undefined, live: boolean): void {
  const flow = useReactFlow();
  const sheet = useFrameState()?.inspector?.parentElement;
  useEffect(() => {
    if (!live || selected === undefined || sheet === null || sheet === undefined || !(globalThis.matchMedia(SHEET).matches || globalThis.matchMedia(PHONE_WIDTH).matches)) {
      return undefined;
    }
    // After the commit that shows the sheet, so its height is there to read.
    const frame = requestAnimationFrame(() => {
      // A card inside a container is placed relative to it: the flow knows where it is on the canvas.
      const card = flow.getInternalNode(selected);
      if (card === undefined) {
        return;
      }
      const { x, y } = card.internals.positionAbsolute;
      const zoom = flow.getZoom();
      const covered = sheet.getBoundingClientRect().height;
      void flow.setCenter(x + (card.measured.width ?? 0) / 2, y + (card.measured.height ?? 0) / 2 + covered / 2 / zoom, { zoom });
    });
    return () => {
      cancelAnimationFrame(frame);
    };
    // Picking a node moves the view; a revision's new placement of the same node does not.
  }, [selected]);
}

function Flow(props: FlowProps) {
  const nodes = useMemo(() => nodesOf(props), [props]);
  const edges = useMemo(() => edgesOf(props), [props]);
  const theme = useResolvedTheme();
  const live = !props.inert;
  useCenterAboveSheet(props.selected, live);
  const wheel = live && props.inScroller !== true;
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
          if (live) {
            props.actions.open?.(node.id);
          }
        }}
        zIndexMode="auto"
        minZoom={ZOOM_MIN}
        maxZoom={ZOOM_MAX}
        // Two-finger scroll pans; pinch (and ctrl+wheel) zooms. The preview does none of it, so a
        // swipe over it scrolls the page; nor does a canvas inside a scrolling page take its wheel.
        panOnDrag={live}
        panOnScroll={wheel}
        zoomOnScroll={false}
        zoomOnPinch={live}
        zoomOnDoubleClick={live}
        preventScrolling={wheel}
        nodesFocusable={live}
        proOptions={{ hideAttribution: true }}
        fitView
        fitViewOptions={FIT}
        colorMode={theme}
        aria-label={props.label}
      >
        {live ? <Controls showInteractive={false} fitViewOptions={FIT} /> : null}
      </ReactFlow>
    </CardActionsContext>
  );
}

/** A graph's canvas: laid-out cards and lines, with pan and zoom (a preview on a phone, MapFrame). */
export function GraphCanvas(props: GraphCanvasProps) {
  return (
    <MapFrame title={props.title}>
      {(inert) => (
        <div className="canvas" data-testid="canvas" data-view={props.viewKey}>
          {/* A new view mounts a new flow, which fits the whole graph (C3); a revision keeps the viewport. */}
          <ReactFlowProvider key={props.viewKey}>
            <Flow {...props} inert={inert} />
          </ReactFlowProvider>
        </div>
      )}
    </MapFrame>
  );
}
