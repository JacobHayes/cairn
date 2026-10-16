// What a canvas shows lives in its address, so a view is shareable, survives a reload, and
// follows the back button. A journey's graph reads the journey's view state (journeys/view.ts:
// the step, the highlights, the Filter and the search, shared with every view) and its own
// settings: the containers the viewer expanded and collapsed since picking the step (`open=`,
// `shut=`, C4), the Signals lens (`lens=`, C6), whether its structure is being edited (5.6), and on
// a phone whether the full-screen map is open. A route's canvas, which has no journey state,
// keeps the same own settings beside its step (`depth=`) and names the container it drilled
// into by `in=`; the parameters it does not own ride along untouched. Every setting at its
// default leaves the address bare.
import { drawnAt, type Placed } from "./ladder.ts";
import { DEFAULT_SETTINGS, LENSES, STEPS, type CanvasSettings, type Lens, type Step } from "./model.ts";
import { DEFAULT_VIEW_STATE, viewStateFrom, viewStateParams, type ViewState } from "../journeys/view.ts";

/**
 * A route canvas's settings, whether the structure is edited, and whether the full-screen map is open (below 720px, where the canvas is otherwise a preview).
 */
export interface CanvasView extends CanvasSettings {
  edit: boolean;
  map: boolean;
  /** The address's other parameters, in order, kept as they were. */
  rest: [string, string][];
}

/** Depth's step (C2). */
const DEPTH = "depth";
/** The containers the viewer expanded and collapsed since picking the step (C4). */
const OPEN = "open";
const SHUT = "shut";
/** A route's canvas: the container drilled into. */
const IN = "in";
const LENS = "lens";
const EDIT = "edit";
/** The full-screen map's address parameter (MapFrame), so Back closes it. */
export const MAP = "map";

/** The parameters a route canvas reads; every other one is left to the page. */
const OWN = new Set([DEPTH, OPEN, SHUT, IN, LENS, EDIT, MAP]);

/** The values of `name`, split on commas, empty when the parameter is there but holds none. */
function listOf(params: URLSearchParams, name: string): string[] | undefined {
  const value = params.get(name);
  return value === null ? undefined : value.split(",").filter(Boolean);
}

/** The earlier canvas's parameters that took other names or other values: see `graphQueryFromOld`. */
export const OLD_CANVAS_PARAMS = ["hide", "in", "notrelevant", "undecided", "heat"];

/** The old canvas's parameters whose meaning changed with the ladder: a bare `trace=on` goes, selecting traces. */
const DROPPED_PARAMS = ["trace"];

/**
 * An earlier canvas query as today's: `in=` becomes the container expanded (`open=`) and `heat=on`
 * the gravity lens. `hide=`, `notrelevant=` and `undecided=` go (a route has no relevance, and its
 * step chooses the kinds drawn), as does `trace=on` (selecting traces). Every other parameter stays.
 */
export function graphQueryFromOld(params: URLSearchParams): URLSearchParams {
  const next = new URLSearchParams();
  if (params.has("in")) {
    next.set(OPEN, params.get("in") ?? "");
  }
  if (params.get("heat") === "on") {
    next.set(LENS, "gravity");
  }
  for (const [name, value] of params) {
    if (!OLD_CANVAS_PARAMS.includes(name) && !DROPPED_PARAMS.includes(name)) {
      next.append(name, value);
    }
  }
  return next;
}

/** A route canvas's settings in an address's query. */
export function viewFrom(params: URLSearchParams): CanvasView {
  const step = params.get(DEPTH);
  return {
    container: params.get(IN) ?? undefined,
    step: STEPS.find((each) => each === step),
    open: listOf(params, OPEN) ?? [],
    shut: listOf(params, SHUT) ?? [],
    lens: lensFrom(params),
    edit: params.get(EDIT) === "on",
    map: params.get(MAP) === "1",
    rest: [...params.entries()].filter(([name]) => !OWN.has(name)),
  };
}

/** `view` as an address's query, leaving out what is at its default. */
export function paramsOf(view: CanvasView): URLSearchParams {
  const params = new URLSearchParams();
  if (view.step !== undefined) {
    params.set(DEPTH, view.step);
  }
  if (view.open.length > 0) {
    params.set(OPEN, view.open.join(","));
  }
  if (view.shut.length > 0) {
    params.set(SHUT, view.shut.join(","));
  }
  if (view.container !== undefined) {
    params.set(IN, view.container);
  }
  if (view.lens !== undefined) {
    params.set(LENS, view.lens);
  }
  if (view.edit) {
    params.set(EDIT, "on");
  }
  if (view.map) {
    params.set(MAP, "1");
  }
  for (const [name, value] of view.rest) {
    params.append(name, value);
  }
  return params;
}

/** The query `view` makes, with its leading `?`, or nothing at the defaults. */
export function searchOf(view: CanvasView): string {
  const text = paramsOf(view).toString();
  return text === "" ? "" : `?${text}`;
}

/** The view a fresh route canvas opens with. */
export const DEFAULT_VIEW: CanvasView = { ...DEFAULT_SETTINGS, edit: false, map: false, rest: [] };

/** What a route canvas is laid out for besides its graph: the step and the container drilled into. Expanding a container keeps it, so the layout keeps its positions (C15). */
export function layoutViewOf(view: Pick<CanvasSettings, "container">, step?: Step): string {
  return JSON.stringify([step ?? null, view.container ?? null]);
}

/** The lens `lens=` names; none when it names none. */
function lensFrom(params: URLSearchParams): Lens | undefined {
  const lens = params.get(LENS);
  return LENSES.find((each) => each === lens);
}

/** A journey graph's own settings: the viewer's expands and collapses, the lens, edit mode and the phone's map. */
export interface GraphSettings {
  open: string[];
  shut: string[];
  lens: Lens | undefined;
  edit: boolean;
  map: boolean;
}

/** What a journey's graph shows: the journey's view state and its own settings. */
export interface GraphView extends GraphSettings {
  state: ViewState;
}

/** The graph's own settings in an address's query. */
export function graphSettingsFrom(params: URLSearchParams): GraphSettings {
  return {
    open: listOf(params, OPEN) ?? [],
    shut: listOf(params, SHUT) ?? [],
    lens: lensFrom(params),
    edit: params.get(EDIT) === "on",
    map: params.get(MAP) === "1",
  };
}

/** What a journey's graph shows, from an address's query. */
export function graphViewFrom(params: URLSearchParams): GraphView {
  return { ...graphSettingsFrom(params), state: viewStateFrom(params) };
}

/** The view a journey's graph opens with: the stock view state, nothing expanded, no lens. */
export const DEFAULT_GRAPH_VIEW: GraphView = { state: DEFAULT_VIEW_STATE, open: [], shut: [], lens: undefined, edit: false, map: false };

/** `view` as an address's query: the view state, then the graph's own settings, leaving out what is at its default. */
function graphParamsOf(view: GraphView): URLSearchParams {
  const params = viewStateParams(view.state);
  if (view.open.length > 0) {
    params.set(OPEN, view.open.join(","));
  }
  if (view.shut.length > 0) {
    params.set(SHUT, view.shut.join(","));
  }
  if (view.lens !== undefined) {
    params.set(LENS, view.lens);
  }
  if (view.edit) {
    params.set(EDIT, "on");
  }
  if (view.map) {
    params.set(MAP, "1");
  }
  return params;
}

function graphSearchOf(view: GraphView): string {
  const text = graphParamsOf(view).toString();
  return text === "" ? "" : `?${text}`;
}

/**
 * C2, 5.7: `view` opened on a node with `ancestors` (its containers, nearest first) and `kind`,
 * with whatever the view hides of it shown: its containers expanded, the least depth from the one
 * chosen that draws it (its kind, and at Stages its top-level node being a stage or a milestone), its class when it is not relevant or conditional, and no Filter or
 * search narrowing (which may not keep it).
 * Frontier and Fade are kept: they hide nothing.
 */
export function revealing(view: GraphView, node: Placed, state: string | undefined): GraphView {
  const depth = view.state.depth ?? "actions";
  const { filter } = view.state;
  return {
    ...view,
    state: {
      ...view.state,
      depth: drawnAt(depth, node) ? view.state.depth : STEPS.slice(STEPS.indexOf(depth)).find((each) => drawnAt(each, node)),
      filter: { mine: false, owner: undefined, flags: [], notRelevant: filter.notRelevant || state === "not_relevant", conditional: filter.conditional || state === "conditional" },
      text: "",
    },
    open: [...new Set([...view.open, ...node.ancestors])],
    shut: view.shut.filter((key) => !node.ancestors.includes(key)),
  };
}

/** The path of a journey's canvas (PLAN, GRAPH), or of a node's detail on it, keeping what the canvas shows. */
export function canvasPath(journey: string, view: GraphView, node?: string): string {
  return `/journeys/${journey}/graph${node === undefined ? "" : `/nodes/${node}`}${graphSearchOf(view)}`;
}

/** An edge's card on the canvas: the ends it joins, `<from>~<to>`, in the address. */
export function edgePath(journey: string, view: GraphView, edge: { from: string; to: string }): string {
  return `/journeys/${journey}/graph/edges/${edge.from}~${edge.to}${graphSearchOf(view)}`;
}
