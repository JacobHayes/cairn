// The canvas itself (C1, C3): React Flow for pan, zoom, nested containers, custom cards, and
// the edges ELK routed (ARCHITECTURE, Web UI: Canvas), over cards and lines already laid out.
// Nothing is dragged (layout is automatic, C15; pinned positions are Later), nothing animates
// (C6), and the view fits when what it shows changes (a step, an expand or a collapse), not on
// every revision, so a live edit never jumps the viewport. It opens fitted to the whole drawn
// graph, as `f` and the Fit button do, unless it is given nodes to open on (`firstFit`).
//
// Input (5.10), 720px and wider: two-finger scroll pans freely, the wheel alone never zooms, a
// pinch or Ctrl/Cmd+wheel zooms at the pointer, a double-click expands a container. Zoom only
// simplifies a container's head below a zoom (`data-lod` on the root, CSS only); a card never changes size.
import "@xyflow/react/dist/base.css";
import "./canvas.css";

import { ControlButton, Controls, ReactFlow, ReactFlowProvider, useNodesInitialized, useReactFlow, useStore, useStoreApi, type EdgeChange, type NodeChange } from "@xyflow/react";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type MouseEvent as ReactMouseEvent, type ReactNode, type RefObject } from "react";

import { useFrameState } from "../shell/frame.tsx";
import { useResolvedTheme } from "../ui/theme.ts";
import { ContextMenu } from "../ui/ContextMenu.tsx";
import { Tip, usePointTip, type PointTip } from "../ui/Tip.tsx";
import { typing } from "../ui/typing.ts";
import { headHeight } from "./cards.ts";
import { EdgeLine, type LineEdge } from "./EdgeLine.tsx";
import { NEAR_ZOOM, firstFrame, fitZoom, listenPinch, minZoomOf, roomOf, shiftInto } from "./gestures.ts";
import { lensChips, lensTiers } from "./lens.ts";
import type { Layout } from "./layout.ts";
import { fadedCards } from "./look.ts";
import type { CanvasModel, Line, Lens } from "./model.ts";
import { CardActionsContext, NodeCard, type CardActions, type CardNode } from "./NodeCard.tsx";
import type { CanvasOverlay } from "./overlay.ts";
import { firstFitCards } from "./refit.ts";
import { lineWaitWords } from "./words.ts";

import { useWork } from "../data/react.ts";

const nodeTypes = { card: NodeCard };
const edgeTypes = { line: EdgeLine };

/** C3: the most the canvas zooms in; the most it zooms out is computed from the graph (5.6). */
const ZOOM_MAX = 1.5;
/** Room kept free round a fitted view (px), under the floating controls: the controls row and Signal above, the zoom stack at the left, Reveal below. */
const PAD = { top: 56, left: 72, right: 24, bottom: 56 };
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
  /** The nodes lit by Frontier: every other card fades, a container staying clear while any card in it is lit, even under a trace. None, or null, fades nothing. */
  emphasis?: ReadonlySet<string> | null | undefined;
  /** The nodes Depth's Fade dims (journeys/requests.ts, `depthFaded`); a card out of this or of `emphasis` is dimmed once. */
  faded?: ReadonlySet<string> | null | undefined;
  selected: string | undefined;
  /** The edge (a line's id) whose card is open. */
  selectedEdge?: string | undefined;
  actions: CardActions;
  /** What the canvas shows; the view fits the graph again when it changes. */
  viewKey: string;
  /** The accessible name of the canvas. */
  label: string;
  /** An edge was clicked, or reached with the keyboard and chosen. */
  onEdge?: ((line: Line) => void) | undefined;
  /** The empty canvas was clicked (a pan or a lasso is not a click). */
  onBackground?: (() => void) | undefined;
  /**
   * The selection when it has several members (`picked`); a card clicked with Shift, Cmd or Ctrl
   * is toggled into it (`toggle`, also Shift+Space on a focused card), and a Shift-drag on the
   * background sets it to what it holds plus the cards in the box (`set`).
   */
  selection?: { picked: ReadonlySet<string>; members: readonly string[]; toggle: (key: string) => void; set: (keys: string[]) => void } | undefined;
  reveal?: Reveal | undefined;
  /** The nodes the view first opens framed on (a proposal's changes, in the rail's order: see `firstFrame`), with the whole graph when none is drawn; none yet (an empty list) holds the first fit back. Fit still fits all. */
  firstFit?: readonly string[] | undefined;
  /** What sits over the canvas: the Signal button, Reveal. A new view mounts it afresh. */
  children?: ReactNode;
  /** What sits over the canvas's top left and outlives a new view: the view's controls, whose open popover a refit must not close. */
  held?: ReactNode;
}

/** Whether a card is a container drawn open, with children to collapse. */
function openContainers(model: CanvasModel): Set<string> {
  return new Set(model.cards.flatMap((card) => (card.parent === undefined ? [] : [card.parent])));
}

/** `props` with `faded` holding every card out of focus for Depth's Fade or for Frontier (`fadedCards`). */
function withEmphasis(props: GraphCanvasProps): GraphCanvasProps {
  const { emphasis, faded, model } = props;
  return emphasis === null || emphasis === undefined ? props : { ...props, faded: fadedCards(model.cards, faded, emphasis) };
}

function nodesOf(props: GraphCanvasProps, edgeEnds: ReadonlySet<string>): CardNode[] {
  const { model, layout, overlay, lens, faded, selected, selection } = props;
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
        picked: selection?.picked.has(card.key) === true,
        header: containers.has(card.key) ? headHeight(card) : undefined,
        place: { x: Math.round(placed.x), y: Math.round(placed.y) },
        lens: chip === undefined || lens === undefined ? undefined : { chip, tier: tiers?.get(card.key) ?? 0, name: lens },
        edgeEnd: edgeEnds.has(card.key),
        collapsible: containers.has(card.key),
      },
      draggable: false,
      // xyflow selects only for the lasso; which nodes are selected is ours (the address and the frame's set).
      selectable: selection !== undefined,
      selected: selection?.members.includes(card.key) === true,
      connectable: false,
    };
    if (card.parent !== undefined) {
      node.parentId = card.parent;
    }
    return [node];
  });
}

/** What a line says (5.7): its sentence, and where the journey's state is known, whether it still holds anything back. */
function lineWords(line: Line): string {
  const state = lineWaitWords(line);
  return state === undefined ? line.sentence : `${line.sentence} \u00b7 ${state}`;
}

function edgesOf({ model, layout, overlay, faded, selectedEdge }: GraphCanvasProps): LineEdge[] {
  return model.lines.map((line) => ({
    id: line.id,
    source: line.from,
    target: line.to,
    type: "line",
    // The line's one accessible name is what hovering or focusing it says.
    ariaLabel: lineWords(line),
    data: {
      line,
      route: layout.routes[line.id],
      overlay,
      satisfied: line.satisfied === true,
      // A line fades with the cards it joins, except a condition (the decisions view keeps those in full).
      faded: line.kind !== "condition" && faded?.has(line.from) === true && faded.has(line.to),
    },
    selected: line.id === selectedEdge,
    // A line the view's own level does not draw (a selection's or Reveal's) keeps its hover and opens no card.
    selectable: line.selectable,
    focusable: line.selectable,
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

/** Whether the inspector is a sheet fixed over the page (below 1100px), not a column beside the workspace. */
function isSheet(column: HTMLElement | null | undefined): column is HTMLElement {
  return column !== null && column !== undefined && getComputedStyle(column).position === "fixed";
}

/** The height (px) the inspector's sheet or its pill lies over the canvas's lower part: none where the canvas ends above it (a phone's, at the peek). */
function coveredBy(column: HTMLElement | null | undefined, canvas: HTMLElement | null | undefined): number {
  if (!isSheet(column) || canvas === null || canvas === undefined) {
    return 0;
  }
  const sheet = column.getBoundingClientRect();
  // A sheet with nothing in it is not drawn: it covers nothing.
  return sheet.height === 0 ? 0 : Math.max(0, canvas.getBoundingClientRect().bottom - sheet.top);
}

/**
 * Writes the canvas's size as it is now to the flow's store, which a fit and the room a move
 * leaves are read from. React Flow learns a new size from its resize observer, which reports on
 * the next frame the browser draws; a fit asked before that frame would fit the old size.
 */
function measureCanvas(store: ReturnType<typeof useStoreApi>): void {
  const node = store.getState().domNode;
  if (node !== null && node.offsetWidth > 0 && node.offsetHeight > 0) {
    store.setState({ width: node.offsetWidth, height: node.offsetHeight });
  }
}

/**
 * Runs `move` on the next frame, counted as the tab's work until it has moved the view (a fit
 * React Flow runs later still included) or is cancelled; returns the cancel.
 */
function onNextFrame(work: (step: Promise<unknown>) => void, move: () => Promise<unknown> | undefined): () => void {
  let cancel: () => void = () => undefined;
  work(
    new Promise<unknown>((resolve) => {
      const frame = requestAnimationFrame(() => {
        resolve(move());
      });
      cancel = () => {
        cancelAnimationFrame(frame);
        resolve(undefined);
      };
    }),
  );
  return () => {
    cancel();
  };
}

/**
 * A fit the viewer asks for (the `f` key, the Fit button, an expanded container): the whole graph,
 * or `node` alone at no more than full size, measured when it runs and padded for the sheet or
 * rail over the canvas's lower part. It answers once React Flow has run it, on a later frame.
 */
function useFit(): (node?: string) => Promise<unknown> {
  const sheet = useFrameState()?.inspector?.parentElement;
  const store = useStoreApi();
  const flow = useReactFlow();
  return useCallback(
    (node?: string) => {
      measureCanvas(store);
      const padding = { ...FIT_PADDING, bottom: px(PAD.bottom + coveredBy(sheet, store.getState().domNode)) };
      return flow.fitView(node === undefined ? { ...FIT, padding } : { nodes: [{ id: node }], padding, maxZoom: 1 });
    },
    [sheet, store, flow],
  );
}

/**
 * Below 1100px the inspector is a sheet over the page, and over the canvas's lower
 * part where the canvas reaches it: a node picked is brought into the band the sheet leaves, less the zones the floating controls
 * keep (PAD), by the least move (none if it is already whole in it), so the selection is never under the sheet and the view keeps its neighbours.
 */
function useCenterAboveSheet(selected: string | undefined, layered: boolean): void {
  const flow = useReactFlow();
  const store = useStoreApi();
  const sheet = useFrameState()?.inspector?.parentElement;
  const work = useWork();
  useEffect(() => {
    // Under a proposal's layer the rail's rows pick nodes the view may not show, in a column as on a sheet.
    if (selected === undefined || ((sheet === null || sheet === undefined || !isSheet(sheet)) && !layered)) {
      return undefined;
    }
    // After the commit that shows the sheet, so its height is there to read.
    return onNextFrame(work, () => {
      // A card inside a container is placed relative to it: the flow knows where it is on the canvas.
      const card = flow.getInternalNode(selected);
      if (card === undefined) {
        return undefined;
      }
      const { x, y } = card.internals.positionAbsolute;
      const zoom = flow.getZoom();
      measureCanvas(store);
      const { width, height, domNode } = store.getState();
      const covered = coveredBy(sheet, domNode);
      // The room the sheet and the controls leave. A card already whole inside it stays where it is, so its neighbours stay in view; one outside is moved by the least that brings it in (centred if it is larger than the room).
      const room = roomOf({ width, height }, PAD, covered);
      const view = flow.getViewport();
      const box = { left: view.x + x * zoom, top: view.y + y * zoom, width: (card.measured.width ?? 0) * zoom, height: (card.measured.height ?? 0) * zoom };
      const [dx, dy] = [shiftInto(box.left, box.width, room.left, room.right), shiftInto(box.top, box.height, room.top, room.bottom)];
      return dx !== 0 || dy !== 0 ? flow.setViewport({ zoom, x: view.x + dx, y: view.y + dy }) : undefined;
    });
    // Picking a node moves the view; a revision's new placement of the same node does not.
  }, [selected]);
}

/** A find centres its result, and an expand or collapse fits the container (5.1): once the card is drawn as asked. */
function useReveal(reveal: Reveal | undefined, model: CanvasModel): void {
  const flow = useReactFlow();
  const fitTo = useFit();
  const done = useRef<string | undefined>(undefined);
  const work = useWork();
  useEffect(() => {
    if (reveal === undefined || done.current === reveal.id) {
      return undefined;
    }
    const container = model.cards.some((card) => card.parent === reveal.key);
    const ready = model.cards.some((card) => card.key === reveal.key) && (reveal.how === "centre" || (reveal.how === "expanded") === container);
    if (!ready) {
      return undefined;
    }
    return onNextFrame(work, () => {
      done.current = reveal.id;
      if (reveal.how !== "centre") {
        return fitTo(reveal.key);
      }
      const card = flow.getInternalNode(reveal.key);
      if (card === undefined) {
        return undefined;
      }
      const { x, y } = card.internals.positionAbsolute;
      return flow.setCenter(x + (card.measured.width ?? 0) / 2, y + (card.measured.height ?? 0) / 2, { zoom: Math.max(flow.getZoom(), 0.6) });
    });
  }, [reveal, model, flow, fitTo, work]);
}

/** The canvas's own key: `f` fits the selection, or the whole graph with none. Depth's keys are the view's (screens/keys.ts). */
function useCanvasKeys(props: GraphCanvasProps): void {
  const fitTo = useFit();
  const { selected } = props;
  const work = useWork();
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      // Not behind the key sheet (a modal dialog), where only its own keys act.
      if (event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey || typing(event.target) || (event.target instanceof Element && event.target.closest("dialog[open]") !== null)) {
        return;
      }
      if (event.key === "f") {
        work(fitTo(selected));
      }
    };
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("keydown", onKey);
    };
  }, [fitTo, selected, work]);
}

/** The line under the pointer, for the ink on its two cards; what it says is the point tip's (the pointer only moves the tip, it never re-renders the cards). */
function useEdgeHover(props: GraphCanvasProps) {
  const tip = usePointTip();
  const [hover, setHover] = useState<Line | undefined>(undefined);
  const move = (event: ReactMouseEvent, edge: LineEdge) => {
    if (edge.data !== undefined) {
      tip.hoverAt(edge.data.line.id, event.clientX, event.clientY, lineWords(edge.data.line));
    }
  };
  return {
    hover,
    focus: useEdgeFocus(props.model, tip),
    handlers: {
      // A click on the line that is already selected changes no selection, so only the click can bring its card back.
      onEdgeClick: (event: ReactMouseEvent, edge: LineEdge) => {
        if (edge.data?.line.selectable === true) {
          props.onEdge?.(edge.data.line);
        } else if (edge.data !== undefined) {
          // A line that opens no card says its words on a tap (touch has no hover).
          tip.tapAt(edge.data.line.id, event.clientX, event.clientY, lineWords(edge.data.line));
        }
      },
      onEdgeMouseEnter: (event: ReactMouseEvent, edge: LineEdge) => {
        setHover(edge.data?.line);
        move(event, edge);
      },
      onEdgeMouseMove: move,
      onEdgeMouseLeave: () => {
        setHover(undefined);
        tip.leave();
      },
    },
  };
}

/** A line focused from the keyboard says the same words; touch and the pointer have their own events. */
function useEdgeFocus(model: CanvasModel, tip: PointTip) {
  return {
    onFocus: (event: React.FocusEvent) => {
      const edge = event.target instanceof Element ? event.target.closest<SVGGElement>(".react-flow__edge") : null;
      const line = edge === null ? undefined : model.lines.find((each) => each.id === edge.dataset["id"]);
      if (edge !== null && line !== undefined && event.target instanceof Element && event.target.matches(":focus-visible")) {
        tip.focusOn(line.id, edge.getBoundingClientRect(), lineWords(line));
      }
    },
    onBlur: (event: React.FocusEvent) => {
      if (event.target instanceof Element && event.target.closest(".react-flow__edge") !== null) {
        tip.blur();
      }
    },
  };
}

/**
 * How far the canvas zooms out, computed from the graph (5.6) so fit-all always fits, and the
 * pinch Safari may send instead of a ctrl-key wheel (5.10).
 */
function useZoomRange(props: GraphCanvasProps, root: RefObject<HTMLDivElement | null>): { minZoom: number; fit: number } {
  const flow = useReactFlow();
  const [width, height] = [useStore((state) => state.width), useStore((state) => state.height)];
  // The room a fit has is what a sheet over the canvas's lower part leaves.
  const covered = coveredBy(useFrameState()?.inspector?.parentElement, useStore((state) => state.domNode));
  // Before the canvas has a size there is nothing to fit.
  const fit = useMemo(() => (width === 0 || height === 0 ? 1 : fitZoom(boundsOf(props.model, props.layout), { width, height: Math.max(1, height - covered) }, PAD)), [props.model, props.layout, width, height, covered]);
  const minZoom = minZoomOf(fit);
  const minZoomNow = useRef(minZoom);
  minZoomNow.current = minZoom;
  useEffect(() => {
    const element = root.current;
    if (element === null) {
      return undefined;
    }
    return listenPinch(element, {
      viewport: () => flow.getViewport(),
      setViewport: (viewport) => void flow.setViewport(viewport),
      minZoom: () => minZoomNow.current,
      maxZoom: () => ZOOM_MAX,
    });
  }, [flow, root]);
  return { minZoom, fit };
}

/**
 * The first fit, once the canvas has a size and its cards are in: the whole drawn graph, or under a proposal's layer the changed cards (`firstFit`, in the rail's order) in the room the sheet or rail leaves (see `firstFrame`).
 */
function useFirstFit(props: GraphCanvasProps, fit: number): void {
  const flow = useReactFlow();
  const [initialized, sized] = [useNodesInitialized(), useStore((state) => state.width > 0 && state.height > 0)];
  const sheet = useFrameState()?.inspector?.parentElement;
  const store = useStoreApi();
  const done = useRef(false);
  const target = firstFitCards(props.firstFit, props.model.cards);
  const work = useWork();
  useEffect(() => {
    // The layer's rail is what the room is measured against, and its changes are what is framed: wait for both (an empty `firstFit` is the changes not yet known), so the cards are not framed under a sheet that is not there yet or before they are drawn.
    if (!initialized || !sized || done.current || (target !== undefined && sheet === undefined) || props.firstFit?.length === 0) {
      return;
    }
    done.current = true;
    measureCanvas(store);
    const { width, height, domNode } = store.getState();
    // Where the inspector's sheet lies over the canvas's lower part, fit into what it leaves.
    const covered = coveredBy(sheet, domNode);
    const boxes = (target ?? []).flatMap((id) => {
      const card = flow.getInternalNode(id);
      return card === undefined ? [] : [{ ...card.internals.positionAbsolute, width: card.measured.width ?? 0, height: card.measured.height ?? 0 }];
    });
    const frame = firstFrame(boxes, roomOf({ width, height }, PAD, covered));
    if (frame !== undefined) {
      void flow.setViewport(frame);
      return;
    }
    work(flow.fitView({ ...FIT, padding: { ...FIT_PADDING, bottom: px(PAD.bottom + covered) }, minZoom: minZoomOf(fit) }));
  }, [initialized, sized, flow, fit, target, sheet, store, work]);
}

/** The zoom, as `--z` on the canvas root, so a zoomed-out head can set its title to a readable size on screen without the cards re-rendering. */
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

/** What xyflow tells us was picked: the lasso's cards, added to the selection as the box moves, or the edge chosen by click or key. */
function useChanges({ selection, model, onEdge, onBackground }: GraphCanvasProps) {
  // A box adds to what was selected when it began; the cards it passes over come and go.
  const lassoing = useRef(false);
  const before = useRef<readonly string[]>([]);
  const boxed = useRef(new Set<string>());
  return {
    onPaneClick: () => {
      onBackground?.();
    },
    onSelectionStart: () => {
      lassoing.current = true;
      before.current = selection?.members ?? [];
      boxed.current = new Set();
    },
    onSelectionEnd: () => { lassoing.current = false; },
    onNodesChange: (changes: NodeChange[]) => {
      // A click on a card selects it in xyflow's terms too; only a box adds, a click is ours (onNodeClick).
      if (selection === undefined || !lassoing.current) {
        return;
      }
      for (const change of changes) {
        if (change.type === "select") {
          if (change.selected) {
            boxed.current.add(change.id);
          } else {
            boxed.current.delete(change.id);
          }
        }
      }
      selection.set([...new Set([...before.current, ...boxed.current])]);
    },
    onEdgesChange: (changes: EdgeChange[]) => {
      // A box also selects the lines between the cards it holds; it is the cards that are wanted.
      if (lassoing.current) {
        return;
      }
      for (const change of changes) {
        const line = change.type === "select" && change.selected ? model.lines.find((each) => each.id === change.id) : undefined;
        if (line?.selectable === true) {
          onEdge?.(line);
        }
      }
    },
  };
}

/** A click on a card: Shift, Cmd or Ctrl adds it to the selection or takes it out; a plain click selects it alone. */
function clicked({ selection, actions }: GraphCanvasProps, event: ReactMouseEvent, key: string): void {
  if (selection !== undefined && (event.shiftKey || event.metaKey || event.ctrlKey)) {
    selection.toggle(key);
  } else {
    actions.open?.(key);
  }
}

/** Expand a collapsed container, or collapse an open one: a double-click, or Enter on the focused card (5.1). True when it did. */
function toggleOf({ model, actions }: GraphCanvasProps): (key: string) => boolean {
  return (key) => {
    const found = model.cards.find((each) => each.key === key);
    if (actions.expand !== undefined && found !== undefined && (found.collapsed || model.cards.some((each) => each.parent === found.key))) {
      actions.expand(found.key, found.collapsed);
      return true;
    }
    return false;
  };
}

/** Input (5.10): two-finger scroll pans; pinch (and ctrl+wheel) zooms; the wheel alone never does. */
const INPUT = {
  panOnDrag: true,
  panOnScroll: true,
  zoomOnScroll: false,
  zoomOnPinch: true,
  zoomOnDoubleClick: false,
  preventScrolling: true,
  nodesFocusable: true,
  // Needed for an edge's click and for the lasso; which nodes are selected stays ours.
  elementsSelectable: true,
  edgesFocusable: true,
} as const;

/** The cards at the ends of the line being hovered and of the one selected: both are drawn in ink, so the card beside says which two it describes. */
function useEdgeEnds(model: CanvasModel, selectedEdge: string | undefined, hover: Line | undefined): ReadonlySet<string> {
  const open = useMemo(() => model.lines.find((line) => line.id === selectedEdge), [model, selectedEdge]);
  return useMemo(() => new Set([hover, open].flatMap((line) => (line === undefined ? [] : [line.from, line.to]))), [hover, open]);
}

/** The node menu: open on a card at a point, offered only where the cards have actions. */
function useNodeMenu(props: GraphCanvasProps) {
  const [menu, setMenu] = useState<{ key: string; at: { x: number; y: number } } | undefined>(undefined);
  const closeMenu = useCallback(() => { setMenu(undefined); }, []);
  const openMenu = (key: string, at: { x: number; y: number }) => { setMenu({ key, at }); };
  return { menu, menus: props.actions.menu, openMenu, closeMenu };
}

/**
 * Keys on a focused card: Enter toggles a container (a button inside it keeps its own Enter),
 * Shift+Space toggles the card in the selection, and the menu key, or Shift+F10, opens its menu
 * beside it, as a right-click does at the pointer.
 */
function cardKeys(toggle: (key: string) => boolean, select: ((key: string) => void) | undefined, openMenu: ((key: string, at: { x: number; y: number }) => void) | undefined) {
  return (event: ReactKeyboardEvent) => {
    const card = (event.target as HTMLElement).closest<HTMLElement>(".react-flow__node");
    if (card === null) {
      return;
    }
    if (select !== undefined && event.key === " " && event.shiftKey) {
      event.preventDefault();
      select(card.dataset["id"] ?? "");
    } else if (openMenu !== undefined && (event.key === "ContextMenu" || (event.key === "F10" && event.shiftKey))) {
      const box = card.getBoundingClientRect();
      event.preventDefault();
      openMenu(card.dataset["id"] ?? "", { x: box.left + 16, y: box.top + 24 });
    } else if (event.key === "Enter" && event.target === card && toggle(card.dataset["id"] ?? "")) {
      // Handled here: the journey's own Enter would press the selected node's button.
      event.preventDefault();
      event.stopPropagation();
    }
  };
}

/** The zoom buttons, a stack at the bottom left (our own, so each carries the shared tooltip, not the browser's); Fit pads for the sheet or rail like the other fits. */
function FitControls() {
  const fitTo = useFit();
  const work = useWork();
  const flow = useReactFlow();
  const [atMin, atMax] = [useStore((state) => state.transform[2] <= state.minZoom), useStore((state) => state.transform[2] >= state.maxZoom)];
  return (
    <Controls showZoom={false} showFitView={false} showInteractive={false} position="bottom-left">
      <Tip tip="Zoom in" kind="name">
        <ControlButton className="react-flow__controls-zoomin" aria-label="Zoom in" disabled={atMax} onClick={() => { void flow.zoomIn(); }}>
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32"><path d="M32 18.133H18.133V32h-4.266V18.133H0v-4.266h13.867V0h4.266v13.867H32z" /></svg>
        </ControlButton>
      </Tip>
      <Tip tip="Zoom out" kind="name">
        <ControlButton className="react-flow__controls-zoomout" aria-label="Zoom out" disabled={atMin} onClick={() => { void flow.zoomOut(); }}>
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 5"><path d="M0 0h32v4.2H0z" /></svg>
        </ControlButton>
      </Tip>
      <Tip tip="Fit the whole graph" kind="name">
        <ControlButton className="react-flow__controls-fitview" aria-label="Fit the whole graph" onClick={() => { work(fitTo()); }}>
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 30"><path d="M3.692 4.63c0-.53.4-.938.939-.938h5.215V0H4.708C2.13 0 0 2.054 0 4.63v5.216h3.692V4.631zM27.354 0h-5.2v3.692h5.17c.53 0 .984.4.984.939v5.215H32V4.631A4.624 4.624 0 0027.354 0zm.954 24.83c0 .532-.4.94-.939.94h-5.215v3.768h5.215c2.577 0 4.631-2.13 4.631-4.707v-5.139h-3.692v5.139zm-23.677.94c-.531 0-.939-.4-.939-.94v-5.138H0v5.139c0 2.577 2.13 4.707 4.708 4.707h5.138V25.77H4.631z" /></svg>
        </ControlButton>
      </Tip>
    </Controls>
  );
}

function Flow(props: GraphCanvasProps) {
  const theme = useResolvedTheme();
  const root = useRef<HTMLDivElement>(null);
  const { hover, focus, handlers } = useEdgeHover(props);
  const edgeEnds = useEdgeEnds(props.model, props.selectedEdge, hover);
  const nodes = useMemo(() => nodesOf(withEmphasis(props), edgeEnds), [props, edgeEnds]);
  const edges = useMemo(() => edgesOf(withEmphasis(props)), [props]);
  const { menu, menus, openMenu, closeMenu } = useNodeMenu(props);
  const near = useStore((state) => state.transform[2] >= NEAR_ZOOM);
  const { minZoom, fit } = useZoomRange(props, root);
  useFirstFit(props, fit);
  useZoomVariable(root);
  useCenterAboveSheet(props.selected, props.firstFit !== undefined);
  useReveal(props.reveal, props.model);
  useCanvasKeys(props);
  const changes = useChanges(props);
  const toggle = toggleOf(props);
  const selection = props.selection;
  return (
    <CardActionsContext value={props.actions}>
      <div
        ref={root}
        className="canvas-root"
        onKeyDown={cardKeys(toggle, selection?.toggle, menus === undefined ? undefined : openMenu)}
        onFocus={focus.onFocus}
        onBlur={focus.onBlur}
      >
        <ReactFlow
          nodes={nodes}
          edges={edges}
          nodeTypes={nodeTypes}
          edgeTypes={edgeTypes}
          nodesDraggable={false}
          nodesConnectable={false}
          {...INPUT}
          selectionKeyCode={selection === undefined ? null : "Shift"}
          multiSelectionKeyCode={null}
          {...changes}
          onNodeClick={(event, node) => { clicked(props, event, node.id); }}
          onNodeDoubleClick={(_, node) => { toggle(node.id); }}
          onNodeContextMenu={(event, node) => {
            if (menus !== undefined) {
              event.preventDefault();
              openMenu(node.id, { x: event.clientX, y: event.clientY });
            }
          }}
          {...handlers}
          zIndexMode="auto"
          minZoom={minZoom}
          maxZoom={ZOOM_MAX}
          proOptions={{ hideAttribution: true }}
          colorMode={theme}
          aria-label={props.label}
          data-lod={near ? "near" : "out"}
        >
          <FitControls />
          {props.children}
        </ReactFlow>
        {menu === undefined || menus === undefined ? null : <ContextMenu at={menu.at} groups={menus.groups(menu.key)} label={`Actions for ${props.actions.title(menu.key)}`} onChoose={(id) => { menus.choose(menu.key, id); }} onClose={closeMenu} />}
      </div>
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
      {props.held === undefined ? null : <div className="canvas-panel canvas-top canvas-held">{props.held}</div>}
    </div>
  );
}
