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

import { Controls, ReactFlow, ReactFlowProvider, useNodesInitialized, useReactFlow, useStore, useStoreApi, type EdgeChange, type NodeChange } from "@xyflow/react";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type MouseEvent as ReactMouseEvent, type ReactNode, type RefObject } from "react";

import { useFrameState } from "../shell/frame.tsx";
import { useResolvedTheme } from "../ui/theme.ts";
import { ContextMenu } from "../ui/ContextMenu.tsx";
import { typing } from "../ui/typing.ts";
import { headHeight } from "./cards.ts";
import { EdgeLine, type LineEdge } from "./EdgeLine.tsx";
import { NEAR_ZOOM, fitZoom, listenPinch, minZoomOf } from "./gestures.ts";
import { lensChips, lensTiers } from "./lens.ts";
import type { Layout } from "./layout.ts";
import { fadedCards } from "./look.ts";
import { STEPS, type CanvasModel, type Line, type Lens, type Step } from "./model.ts";
import { CardActionsContext, NodeCard, type CardActions, type CardNode } from "./NodeCard.tsx";
import type { CanvasOverlay } from "./overlay.ts";
import { firstFitCards } from "./refit.ts";

import { nodeTitle } from "../ui/gone.ts";

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
  /** The nodes the view first opens fitted to (a proposal's changes), with the whole graph when none is drawn; Fit still fits all. */
  firstFit?: readonly string[] | undefined;
  /** The ladder's steps, for keys 1 to 4. */
  steps?: readonly Step[];
  onStep?: ((step: Step) => void) | undefined;
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

function edgesOf({ model, layout, overlay, faded, selectedEdge }: GraphCanvasProps): LineEdge[] {
  const cards = new Map(model.cards.map((card) => [card.key, card]));
  return model.lines.map((line) => ({
    id: line.id,
    source: line.from,
    target: line.to,
    type: "line",
    // The flow's own label names the keys of its two ends; the cards' titles say it to a reader.
    ariaLabel: `Edge from ${nodeTitle(cards.get(line.from)?.title)} to ${nodeTitle(cards.get(line.to)?.title)}`,
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

/** The padding a manual fit keeps, read when it runs: FIT_PADDING, and more at the bottom under the sheet or rail. */
function useFitPadding(): () => typeof FIT_PADDING {
  const sheet = useFrameState()?.inspector?.parentElement;
  const store = useStoreApi();
  return useCallback(() => ({ ...FIT_PADDING, bottom: px(PAD.bottom + coveredBy(sheet, store.getState().domNode)) }), [sheet, store]);
}

/**
 * Below 1100px the inspector is a sheet over the page, and over the canvas's lower
 * part where the canvas reaches it: a node picked is brought into the band the sheet leaves, less the zones the floating controls
 * keep (PAD), by the least move (none if it is already whole in it), so the selection is never under the sheet and the view keeps its neighbours.
 */
function useCenterAboveSheet(selected: string | undefined): void {
  const flow = useReactFlow();
  const store = useStoreApi();
  const sheet = useFrameState()?.inspector?.parentElement;
  useEffect(() => {
    if (selected === undefined || sheet === null || sheet === undefined) {
      return undefined;
    }
    // After the commit that shows the sheet, so its height is there to read.
    const frame = requestAnimationFrame(() => {
      if (!isSheet(sheet)) {
        return;
      }
      // A card inside a container is placed relative to it: the flow knows where it is on the canvas.
      const card = flow.getInternalNode(selected);
      if (card === undefined) {
        return;
      }
      const { x, y } = card.internals.positionAbsolute;
      const zoom = flow.getZoom();
      const { width, height, domNode } = store.getState();
      const covered = coveredBy(sheet, domNode);
      // The room the sheet and the controls leave. A card already whole inside it stays where it is, so its neighbours stay in view; one outside is moved by the least that brings it in (centred if it is larger than the room).
      const room = { left: PAD.left, right: width - PAD.right, top: PAD.top, bottom: height - covered - PAD.bottom };
      const view = flow.getViewport();
      const box = { left: view.x + x * zoom, top: view.y + y * zoom, width: (card.measured.width ?? 0) * zoom, height: (card.measured.height ?? 0) * zoom };
      const shift = (from: number, size: number, low: number, high: number) => (size >= high - low ? (low + high) / 2 - (from + size / 2) : from < low ? low - from : from + size > high ? high - (from + size) : 0);
      const [dx, dy] = [shift(box.left, box.width, room.left, room.right), shift(box.top, box.height, room.top, room.bottom)];
      if (dx !== 0 || dy !== 0) {
        void flow.setViewport({ zoom, x: view.x + dx, y: view.y + dy });
      }
    });
    return () => {
      cancelAnimationFrame(frame);
    };
    // Picking a node moves the view; a revision's new placement of the same node does not.
  }, [selected]);
}

/** A find centres its result, and an expand or collapse fits the container (5.1): once the card is drawn as asked. */
function useReveal(reveal: Reveal | undefined, model: CanvasModel): void {
  const flow = useReactFlow();
  const padding = useFitPadding();
  const done = useRef<string | undefined>(undefined);
  useEffect(() => {
    if (reveal === undefined || done.current === reveal.id) {
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
        void flow.fitView({ nodes: [{ id: reveal.key }], padding: padding(), maxZoom: 1 });
      }
    });
    return () => {
      cancelAnimationFrame(frame);
    };
  }, [reveal, model, flow, padding]);
}

/** Keys on the canvas: 1 to 4 pick a step, `f` fits the selection, or the whole graph with none. */
function useCanvasKeys(props: GraphCanvasProps): void {
  const flow = useReactFlow();
  const padding = useFitPadding();
  const { steps, onStep, selected } = props;
  useEffect(() => {
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
        void flow.fitView(selected === undefined ? { ...FIT, padding: padding() } : { nodes: [{ id: selected }], padding: padding(), maxZoom: 1 });
      }
    };
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("keydown", onKey);
    };
  }, [flow, steps, onStep, selected, padding]);
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

/** The first fit, once the canvas has a size and its cards are in: the `firstFit` nodes when any is drawn, else the whole drawn graph. */
function useFirstFit(props: GraphCanvasProps, fit: number): void {
  const flow = useReactFlow();
  const [initialized, sized] = [useNodesInitialized(), useStore((state) => state.width > 0 && state.height > 0)];
  const sheet = useFrameState()?.inspector?.parentElement;
  const store = useStoreApi();
  const done = useRef(false);
  useEffect(() => {
    if (!initialized || !sized || done.current) {
      return;
    }
    done.current = true;
    // Where the inspector's sheet lies over the canvas's lower part, fit into what it leaves.
    const covered = coveredBy(sheet, store.getState().domNode);
    const padding = { ...FIT_PADDING, bottom: px(PAD.bottom + covered) };
    const target = firstFitCards(props.firstFit, props.model.cards);
    void flow.fitView({ ...FIT, padding, minZoom: minZoomOf(fit), ...(target === undefined ? {} : { nodes: target.map((id) => ({ id })) }) });
  }, [initialized, sized, flow, fit, props.firstFit, props.model.cards]);
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
function useChanges({ selection, model, selectedEdge, onEdge, onBackground }: GraphCanvasProps) {
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
        if (line !== undefined && line.id !== selectedEdge) {
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

/** What hovering a line says (5.7): its sentence, and whether it still waits. */
function EdgeChip({ chip, line }: { chip: RefObject<HTMLDivElement | null>; line: Line }) {
  return (
    <div ref={chip} className="edge-hover" role="status" data-testid="edge-hover">
      {line.sentence}
      {line.kind === "dates" ? "" : ` \u00b7 ${line.satisfied === true ? "done" : "still waiting"}`}
    </div>
  );
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

/** The zoom buttons, a stack at the bottom left; Fit pads for the sheet or rail like the other fits. */
function FitControls() {
  const flow = useReactFlow();
  const padding = useFitPadding();
  return (
    <Controls showInteractive={false} position="bottom-left" onFitView={() => void flow.fitView({ ...FIT, padding: padding() })} />
  );
}

function Flow(props: GraphCanvasProps) {
  const theme = useResolvedTheme();
  const root = useRef<HTMLDivElement>(null);
  const { chip, hover, move, enter, leave } = useEdgeHover(root);
  const edgeEnds = useMemo(() => new Set(hover === undefined ? [] : [hover.from, hover.to]), [hover]);
  const nodes = useMemo(() => nodesOf(withEmphasis(props), edgeEnds), [props, edgeEnds]);
  const edges = useMemo(() => edgesOf(withEmphasis(props)), [props]);
  const { menu, menus, openMenu, closeMenu } = useNodeMenu(props);
  const near = useStore((state) => state.transform[2] >= NEAR_ZOOM);
  const { minZoom, fit } = useZoomRange(props, root);
  useFirstFit(props, fit);
  useZoomVariable(root);
  useCenterAboveSheet(props.selected);
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
          onEdgeMouseEnter={(event, edge) => { enter(event, edge.data?.line); }}
          onEdgeMouseMove={move}
          onEdgeMouseLeave={leave}
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
        {hover === undefined ? null : <EdgeChip chip={chip} line={hover} />}
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
