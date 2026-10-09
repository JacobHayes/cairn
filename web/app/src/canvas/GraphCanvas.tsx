// The canvas itself (C1, C3): React Flow for pan, zoom, nested containers, custom cards, and
// the edges ELK routed (ARCHITECTURE, Web UI: Canvas), over cards and lines already laid out.
// Nothing is dragged (layout is automatic, C15; pinned positions are Later), nothing animates
// (C6), and the view fits when what it shows changes (a step, an expand or a collapse), not on
// every revision, so a live edit never jumps the viewport. It fits the whole graph when that is
// readable (0.6 or more), else the acting area at 0.6 (C5); `f` and the Fit button fit all.
//
// Input (5.10), 720px and wider: two-finger scroll pans freely, the wheel alone never zooms, a
// pinch or Ctrl/Cmd+wheel zooms at the pointer, a double-click expands a container. Zoom only
// changes how much each card draws, through three bands on the root (`data-lod`, CSS only).
import "@xyflow/react/dist/base.css";
import "./canvas.css";

import { Controls, ReactFlow, ReactFlowProvider, useNodesInitialized, useReactFlow, useStore, useStoreApi, type EdgeChange, type NodeChange } from "@xyflow/react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState, type MouseEvent as ReactMouseEvent, type ReactNode, type RefObject } from "react";

import { PHONE_WIDTH, SHEET, useFrameState } from "../shell/frame.tsx";
import { useResolvedTheme } from "../ui/theme.ts";
import { typing } from "../ui/typing.ts";
import { cardRows, cardHeight } from "./cards.ts";
import { EdgeLine, type LineEdge } from "./EdgeLine.tsx";
import { NEAR_ZOOM, bandOf, fitZoom, listenPinch, minZoomOf } from "./gestures.ts";
import { lensChips, lensTiers } from "./lens.ts";
import type { Layout } from "./layout.ts";
import { MapFrame } from "./MapFrame.tsx";
import { STEPS, type CanvasModel, type Line, type Lens, type Step } from "./model.ts";
import { CardActionsContext, NodeCard, type CardActions, type CardNode } from "./NodeCard.tsx";
import { standsFor, type CanvasOverlay } from "./overlay.ts";

const nodeTypes = { card: NodeCard };
const edgeTypes = { line: EdgeLine };

/** C3: the most the canvas zooms in; the most it zooms out is computed from the graph (5.6). */
const ZOOM_MAX = 1.5;
/** Room kept free round a fitted view (px): the ladder's row above, the zoom controls and the hidden count below. */
const PAD = { top: 64, left: 56, right: 24, bottom: 56 };
const px = (pixels: number): `${number}px` => `${String(pixels) as `${number}`}px`;
const FIT_PADDING = { top: px(PAD.top), left: px(PAD.left), right: px(PAD.right), bottom: px(PAD.bottom) };
/** A small graph fits at 100% rather than blown up to fill the region. */
const FIT = { maxZoom: 1, padding: FIT_PADDING };

/** What a navigation asks the view to do once the card is there: centre on it (a find), or fit to the container just expanded or collapsed (5.1). */
export interface Reveal {
  key: string;
  /** The navigation that asked, so each is done once. */
  id: string;
  how: "centre" | "expanded" | "collapsed";
}

export interface GraphCanvasProps {
  model: CanvasModel;
  layout: Layout;
  overlay: CanvasOverlay | undefined;
  lens: Lens | undefined;
  /** View, Show origins. */
  origins?: boolean;
  /** Cards a filter fades; the trace decides instead while there is one. */
  faded?: ReadonlySet<string> | undefined;
  selected: string | undefined;
  /** The edge (a line's id) whose card is open. */
  selectedEdge?: string | undefined;
  actions: CardActions;
  /** What the canvas shows; the view fits the graph again when it changes. */
  viewKey: string;
  /** The accessible name of the canvas. */
  label: string;
  /** What the map is of, as its full-screen bar shows it (the journey's name, not the label). */
  title: string;
  /** The canvas sits inside a page that scrolls: the wheel scrolls the page, and drag and pinch still pan and zoom. */
  inScroller?: boolean;
  /** An edge was clicked, or reached with the keyboard and chosen. */
  onEdge?: ((line: Line) => void) | undefined;
  /** The Select mode (5.9): shift-click and shift-drag pick cards. */
  selecting?: { picked: ReadonlySet<string>; onPicked: (keys: string[]) => void } | undefined;
  reveal?: Reveal | undefined;
  /** Where the view opens when the whole graph is too small to read (C5): groups of nodes, the widest first (the current stages and the acting frontier, then the stages). */
  focus?: readonly (readonly string[])[] | undefined;
  /** The ladder's steps, for keys 1 to 4. */
  steps?: readonly Step[];
  onStep?: ((step: Step) => void) | undefined;
  /** What sits over the canvas: the ladder, the View menu, the hidden count. */
  children?: ReactNode;
}

/** `inert`: the phone's preview, which takes no gesture and opens nothing (MapFrame). */
interface FlowProps extends GraphCanvasProps {
  inert: boolean;
}

/** Whether a card is a container drawn open, with children to collapse. */
function openContainers(model: CanvasModel): Set<string> {
  return new Set(model.cards.flatMap((card) => (card.parent === undefined ? [] : [card.parent])));
}

function nodesOf(props: FlowProps, edgeEnds: ReadonlySet<string>): CardNode[] {
  const { model, layout, overlay, lens, faded, selected, selecting } = props;
  const containers = openContainers(model);
  const chips = lens === undefined ? undefined : lensChips(model.cards, lens);
  const tiers = chips === undefined ? undefined : lensTiers(chips);
  return model.cards.flatMap((card) => {
    const placed = layout.nodes[card.key];
    if (placed === undefined) {
      return [];
    }
    const chip = chips?.get(card.key);
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
        faded: faded?.has(card.key) === true,
        selected: card.key === selected,
        picked: selecting?.picked.has(card.key) === true,
        header: containers.has(card.key) ? cardHeight(cardRows(card)) : undefined,
        place: { x: Math.round(placed.x), y: Math.round(placed.y) },
        lens: chip === undefined || lens === undefined ? undefined : { chip, tier: tiers?.get(card.key) ?? 0, name: lens },
        edgeEnd: edgeEnds.has(card.key),
        collapsible: containers.has(card.key),
        origins: props.origins === true,
      },
      draggable: false,
      // xyflow selects only for the Select mode's lasso; which node is open is ours (the address).
      selectable: selecting !== undefined,
      selected: selecting?.picked.has(card.key) === true,
      connectable: false,
    };
    if (card.parent !== undefined) {
      node.parentId = card.parent;
    }
    return [node];
  });
}

function edgesOf({ model, layout, overlay, faded, selectedEdge }: FlowProps): LineEdge[] {
  return model.lines.map((line) => ({
    id: line.id,
    source: line.from,
    target: line.to,
    type: "line",
    data: {
      line,
      route: layout.routes[line.id],
      overlay,
      satisfied: line.satisfied === true,
      // A line fades with the cards it joins, except a condition (the decisions view keeps those in full).
      faded: line.kind !== "condition" && faded?.has(line.from) === true && faded.has(line.to),
    },
    selected: line.id === selectedEdge,
  }));
}

/** The graph's size on the canvas: the top-level cards' outer edges. */
function boundsOf(model: CanvasModel, layout: Layout): { width: number; height: number } {
  let [width, height] = [0, 0];
  for (const card of model.cards) {
    const placed = card.parent === undefined ? layout.nodes[card.key] : undefined;
    if (placed !== undefined) {
      width = Math.max(width, placed.x + placed.width);
      height = Math.max(height, placed.y + placed.height);
    }
  }
  return { width, height };
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

/** A find centres its result, and an expand or collapse fits the container (5.1): once the card is drawn as asked. */
function useReveal(reveal: Reveal | undefined, model: CanvasModel, live: boolean): void {
  const flow = useReactFlow();
  const done = useRef<string | undefined>(undefined);
  useEffect(() => {
    if (!live || reveal === undefined || done.current === reveal.id) {
      return undefined;
    }
    const container = model.cards.some((card) => card.parent === reveal.key);
    const ready = model.cards.some((card) => card.key === reveal.key) && (reveal.how === "centre" || (reveal.how === "expanded") === container);
    if (!ready) {
      return undefined;
    }
    const frame = requestAnimationFrame(() => {
      done.current = reveal.id;
      if (reveal.how === "centre") {
        const card = flow.getInternalNode(reveal.key);
        if (card !== undefined) {
          const { x, y } = card.internals.positionAbsolute;
          void flow.setCenter(x + (card.measured.width ?? 0) / 2, y + (card.measured.height ?? 0) / 2, { zoom: Math.max(flow.getZoom(), 0.6) });
        }
      } else {
        void flow.fitView({ nodes: [{ id: reveal.key }], padding: FIT_PADDING, maxZoom: 1 });
      }
    });
    return () => {
      cancelAnimationFrame(frame);
    };
  }, [reveal, model, live, flow]);
}

/** Keys on the canvas: 1 to 4 pick a step, `f` fits the selection, or the whole graph with none. */
function useCanvasKeys(props: FlowProps, live: boolean): void {
  const flow = useReactFlow();
  const { steps, onStep, selected } = props;
  useEffect(() => {
    if (!live) {
      return undefined;
    }
    const onKey = (event: KeyboardEvent) => {
      // Not behind the key sheet (a modal dialog), where only its own keys act.
      if (event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey || typing(event.target) || (event.target instanceof Element && event.target.closest("dialog[open]") !== null)) {
        return;
      }
      const step = /^[1-4]$/.test(event.key) ? STEPS[Number(event.key) - 1] : undefined;
      if (step !== undefined) {
        if (steps?.includes(step) === true) {
          onStep?.(step);
        }
      } else if (event.key === "f") {
        void flow.fitView(selected === undefined ? FIT : { nodes: [{ id: selected }], padding: FIT_PADDING, maxZoom: 1 });
      }
    };
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("keydown", onKey);
    };
  }, [flow, steps, onStep, selected, live]);
}

/** The line under the pointer, and the chip that says what it is: where the pointer is only moves the chip, it never re-renders the cards. */
function useEdgeHover(root: RefObject<HTMLDivElement | null>) {
  const chip = useRef<HTMLDivElement>(null);
  const pointer = useRef({ x: 0, y: 0 });
  const [hover, setHover] = useState<Line | undefined>(undefined);
  const place = () => {
    chip.current?.style.setProperty("left", `${String(pointer.current.x + 12)}px`);
    chip.current?.style.setProperty("top", `${String(pointer.current.y + 12)}px`);
  };
  useLayoutEffect(place, [hover]);
  const move = (event: ReactMouseEvent) => {
    const box = root.current?.getBoundingClientRect();
    pointer.current = { x: event.clientX - (box?.left ?? 0), y: event.clientY - (box?.top ?? 0) };
    place();
  };
  return {
    chip,
    hover,
    move,
    enter: (event: ReactMouseEvent, line: Line | undefined) => {
      move(event);
      setHover(line);
    },
    leave: () => {
      setHover(undefined);
    },
  };
}

/**
 * How far the canvas zooms out, computed from the graph (5.6) so fit-all always fits, and the
 * pinch Safari may send instead of a ctrl-key wheel (5.10).
 */
function useZoomRange(props: FlowProps, live: boolean, root: RefObject<HTMLDivElement | null>): { minZoom: number; fit: number } {
  const flow = useReactFlow();
  const [width, height] = [useStore((state) => state.width), useStore((state) => state.height)];
  // Before the canvas has a size there is nothing to fit.
  const fit = useMemo(() => (width === 0 || height === 0 ? 1 : fitZoom(boundsOf(props.model, props.layout), { width, height }, PAD)), [props.model, props.layout, width, height]);
  const minZoom = minZoomOf(fit);
  const minZoomNow = useRef(minZoom);
  minZoomNow.current = minZoom;
  useEffect(() => {
    const element = root.current;
    if (!live || element === null) {
      return undefined;
    }
    return listenPinch(element, {
      viewport: () => flow.getViewport(),
      setViewport: (viewport) => void flow.setViewport(viewport),
      minZoom: () => minZoomNow.current,
      maxZoom: () => ZOOM_MAX,
    });
  }, [flow, live, root]);
  return { minZoom, fit };
}

/**
 * The first fit, once the canvas has a size and its cards are in: the whole graph when it is
 * readable at that zoom, else the first group of the focus that fits at the readable zoom (or
 * the last, cropped), so the view opens on where the work is (C5) rather than on titles too
 * small to read.
 */
function useFirstFit(props: FlowProps, fit: number, live: boolean): void {
  const flow = useReactFlow();
  const store = useStoreApi();
  const [initialized, sized] = [useNodesInitialized(), useStore((state) => state.width > 0 && state.height > 0)];
  const sheet = useFrameState()?.inspector?.parentElement;
  const done = useRef(false);
  useEffect(() => {
    if (!initialized || !sized || done.current) {
      return;
    }
    done.current = true;
    const { width, height } = store.getState();
    // On a tablet the inspector's sheet lies over the canvas's lower part: fit into what it leaves.
    const covered = sheet !== null && sheet !== undefined && globalThis.matchMedia(SHEET).matches ? sheet.getBoundingClientRect().height : 0;
    const padding = { ...FIT_PADDING, bottom: px(PAD.bottom + covered) };
    const room = { width, height: height - covered };
    const groups = (live && fit < NEAR_ZOOM ? (props.focus ?? []) : []).map((keys) => props.model.cards.filter((card) => standsFor(card).some((key) => keys.includes(key))).map((card) => card.key)).filter((ids) => ids.length > 0);
    const near = groups.find((ids) => fitZoom(flow.getNodesBounds(ids), room, PAD) >= NEAR_ZOOM) ?? groups.at(-1);
    void flow.fitView(near === undefined ? { ...FIT, padding, minZoom: minZoomOf(fit) } : { nodes: near.map((id) => ({ id })), padding, minZoom: NEAR_ZOOM, maxZoom: 1 });
  }, [initialized, sized, flow, store, fit, live, props.model, props.focus]);
}

/** The zoom, as `--z` on the canvas root, so the far bands can set a title to a readable size on screen without the cards re-rendering. */
function useZoomVariable(root: RefObject<HTMLDivElement | null>): void {
  const store = useStoreApi();
  useLayoutEffect(() => {
    let last: number | undefined;
    const set = (zoom: number) => {
      if (zoom !== last) {
        last = zoom;
        root.current?.style.setProperty("--z", String(zoom));
      }
    };
    set(store.getState().transform[2]);
    return store.subscribe((state) => { set(state.transform[2]); });
  }, [store, root]);
}

/** What xyflow tells us was picked: the lasso's cards in the Select mode, or the edge chosen by click or key. */
function useChanges({ selecting, model, selectedEdge, onEdge }: FlowProps) {
  return {
    onNodesChange: (changes: NodeChange[]) => {
      if (selecting === undefined) {
        return;
      }
      const picked = new Set(selecting.picked);
      for (const change of changes) {
        if (change.type === "select") {
          if (change.selected) {
            picked.add(change.id);
          } else {
            picked.delete(change.id);
          }
        }
      }
      selecting.onPicked([...picked]);
    },
    onEdgesChange: (changes: EdgeChange[]) => {
      for (const change of changes) {
        const line = change.type === "select" && change.selected ? model.lines.find((each) => each.id === change.id) : undefined;
        if (line !== undefined && line.id !== selectedEdge) {
          onEdge?.(line);
        }
      }
    },
  };
}

/** Expand a collapsed container, or collapse an open one: a double-click, or Enter on the focused card (5.1). True when it did. */
function toggleOf({ model, actions }: FlowProps, live: boolean): (key: string) => boolean {
  return (key) => {
    const found = model.cards.find((each) => each.key === key);
    if (live && actions.expand !== undefined && found !== undefined && (found.collapsed || model.cards.some((each) => each.parent === found.key))) {
      actions.expand(found.key, found.collapsed);
      return true;
    }
    return false;
  };
}

/**
 * Input (5.10): two-finger scroll pans; pinch (and ctrl+wheel) zooms; the wheel alone never does.
 * The preview does none of it, so a swipe over it scrolls the page; nor does a canvas inside a
 * scrolling page take its wheel.
 */
function inputOf(live: boolean, inScroller: boolean) {
  const wheel = live && !inScroller;
  return {
    panOnDrag: live,
    panOnScroll: wheel,
    zoomOnScroll: false,
    zoomOnPinch: live,
    zoomOnDoubleClick: false,
    preventScrolling: wheel,
    nodesFocusable: live,
    // Needed for an edge's click and for the Select mode's lasso; which node is open stays ours.
    elementsSelectable: live,
    edgesFocusable: live,
  };
}

/** What hovering a line says (5.7): its sentence, and whether it still waits. */
function EdgeChip({ chip, line }: { chip: RefObject<HTMLDivElement | null>; line: Line }) {
  return (
    <div ref={chip} className="edge-hover" role="status" data-testid="edge-hover">
      {line.sentence}
      {line.kind === "dates" ? "" : ` \u00b7 ${line.satisfied === true ? "done" : "still waiting"}`}
    </div>
  );
}

function Flow(props: FlowProps) {
  const theme = useResolvedTheme();
  const live = !props.inert;
  const root = useRef<HTMLDivElement>(null);
  const { chip, hover, move, enter, leave } = useEdgeHover(root);
  const edgeEnds = useMemo(() => new Set(hover === undefined ? [] : [hover.from, hover.to]), [hover]);
  const nodes = useMemo(() => nodesOf(props, edgeEnds), [props, edgeEnds]);
  const edges = useMemo(() => edgesOf(props), [props]);
  const band = useStore((state) => bandOf(state.transform[2]));
  const { minZoom, fit } = useZoomRange(props, live, root);
  useFirstFit(props, fit, live);
  useZoomVariable(root);
  useCenterAboveSheet(props.selected, live);
  useReveal(props.reveal, props.model, live);
  useCanvasKeys(props, live);
  const { onNodesChange, onEdgesChange } = useChanges(props);
  const toggle = toggleOf(props, live);
  const selecting = props.selecting;
  return (
    <CardActionsContext value={props.actions}>
      <div
        ref={root}
        className="canvas-root"
        onKeyDown={(event) => {
          // Enter on a focused card toggles it (a button inside it keeps its own Enter).
          const card = (event.target as HTMLElement).closest<HTMLElement>(".react-flow__node");
          if (event.key === "Enter" && card !== null && event.target === card && toggle(card.dataset["id"] ?? "")) {
            // Handled here: the journey's own Enter would press the selected node's button.
            event.preventDefault();
            event.stopPropagation();
          }
        }}
      >
        <ReactFlow
          nodes={nodes}
          edges={edges}
          nodeTypes={nodeTypes}
          edgeTypes={edgeTypes}
          nodesDraggable={false}
          nodesConnectable={false}
          {...inputOf(live, props.inScroller === true)}
          selectionKeyCode={selecting === undefined ? null : "Shift"}
          multiSelectionKeyCode={selecting === undefined ? null : "Shift"}
          onNodesChange={onNodesChange}
          onEdgesChange={onEdgesChange}
          onNodeClick={(event, node) => {
            if (live && !(selecting !== undefined && event.shiftKey)) {
              props.actions.open?.(node.id);
            }
          }}
          onNodeDoubleClick={(_, node) => { toggle(node.id); }}
          onEdgeMouseEnter={(event, edge) => { enter(event, edge.data?.line); }}
          onEdgeMouseMove={move}
          onEdgeMouseLeave={leave}
          zIndexMode="auto"
          minZoom={minZoom}
          maxZoom={ZOOM_MAX}
          proOptions={{ hideAttribution: true }}
          colorMode={theme}
          aria-label={props.label}
          data-lod={band}
        >
          {live ? <Controls showInteractive={false} position="bottom-left" fitViewOptions={FIT} /> : null}
          {live ? props.children : null}
        </ReactFlow>
        {hover === undefined ? null : <EdgeChip chip={chip} line={hover} />}
      </div>
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
