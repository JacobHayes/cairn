// What the canvas shows lives in its address, so a view is shareable, survives a reload, and
// follows the back button: the detail step (`detail=`, C2), the containers the viewer expanded
// and collapsed since (`open=`, `shut=`, C4), the kinds kept at full strength (`kind=`, the
// others fade), whether not-relevant and conditional nodes are shown (`show=`, C1), the
// Signals lens (`lens=`, C6), on a journey whether its structure is being edited (5.6) or
// nodes are being selected (5.9), and on a phone whether the full-screen map is open. The
// canvas lives at `/journeys/<id>/plan/graph`; every parameter it does not own (the toolbar's
// `decisions`, `mine`, `q`) rides along untouched, so a link made from the canvas keeps the
// chips. Every setting at its default leaves the address bare. A route's canvas, which has no
// ladder, names the container it drilled into by `in=`.
import type { ProjectionRequest } from "@cairn/wasm";

import type { NodeKind } from "../detail/model.ts";
import { kindsAt } from "./ladder.ts";
import { DEFAULT_SETTINGS, KINDS, LENSES, STEPS, type CanvasSettings, type LevelDisplay, type Step } from "./model.ts";

/**
 * The canvas's settings, whether the structure is edited, whether nodes are being selected, and
 * whether the full-screen map is open (below 720px, where the canvas is otherwise a preview).
 */
export interface CanvasView extends CanvasSettings {
  edit: boolean;
  select: boolean;
  map: boolean;
  /** The address's other parameters, in order, kept as they were. */
  rest: [string, string][];
}

/** The detail step (C2). */
const DETAIL = "detail";
/** The containers the viewer expanded and collapsed since picking the step (C4). */
const OPEN = "open";
const SHUT = "shut";
/** The kinds kept at full strength (C2). */
const KIND = "kind";
/** A route's canvas: the container drilled into. */
const IN = "in";
/** What is shown beyond the live nodes: `notrelevant` and `conditional` (C1). */
const SHOW = "show";
const LENS = "lens";
const ORIGINS = "origins";
const EDIT = "edit";
const SELECT = "select";
/** The full-screen map's address parameter (MapFrame), so Back closes it. */
export const MAP = "map";

/** The parameters the canvas reads; every other one is left to the toolbar and the page. */
const OWN = new Set([DETAIL, OPEN, SHUT, KIND, IN, SHOW, LENS, ORIGINS, EDIT, SELECT, MAP]);

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
 * An earlier canvas query as today's: `hide=` becomes the kinds left at full strength (`kind=`),
 * `in=` the container expanded (`open=`), `notrelevant=hide` and `undecided=hide` what is shown
 * (`show=`; the earlier canvas showed both unless told not to, so an address that names either
 * keeps what it showed), and `heat=on` the gravity lens.
 * `trace=on` goes: selecting a node traces it. Every other parameter stays.
 */
export function graphQueryFromOld(params: URLSearchParams): URLSearchParams {
  const next = new URLSearchParams();
  const hidden = new Set((params.get("hide") ?? "").split(",").filter(Boolean));
  if (hidden.size > 0) {
    next.set(KIND, KINDS.filter((kind) => !hidden.has(kind)).join(","));
  }
  if (params.has("in")) {
    next.set(OPEN, params.get("in") ?? "");
  }
  // An address that named a relevance toggle keeps its view; one that did not takes today's defaults.
  const notRelevant = params.get("notrelevant") !== "hide";
  const conditional = params.get("undecided") !== "hide";
  if (params.has("notrelevant") || params.has("undecided")) {
    next.set(SHOW, [notRelevant ? "notrelevant" : "", conditional ? "conditional" : ""].filter(Boolean).join(","));
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

/** The canvas's settings in an address's query. */
export function viewFrom(params: URLSearchParams): CanvasView {
  const kinds = listOf(params, KIND);
  const shown = listOf(params, SHOW);
  const step = params.get(DETAIL);
  const lens = params.get(LENS);
  return {
    shown: kinds === undefined ? KINDS : KINDS.filter((kind) => kinds.includes(kind)),
    container: params.get(IN) ?? undefined,
    step: STEPS.find((each) => each === step),
    open: listOf(params, OPEN) ?? [],
    shut: listOf(params, SHUT) ?? [],
    // Settled not-relevant nodes are hidden unless asked for; conditional ones are drawn.
    notRelevant: shown?.includes("notrelevant") ?? DEFAULT_SETTINGS.notRelevant,
    undecided: shown === undefined ? DEFAULT_SETTINGS.undecided : shown.includes("conditional"),
    lens: LENSES.find((each) => each === lens),
    origins: params.get(ORIGINS) === "1",
    edit: params.get(EDIT) === "on",
    select: params.get(SELECT) === "1",
    map: params.get(MAP) === "1",
    rest: [...params.entries()].filter(([name]) => !OWN.has(name)),
  };
}

/** `view` as an address's query, leaving out what is at its default. */
export function paramsOf(view: CanvasView): URLSearchParams {
  const params = new URLSearchParams();
  if (view.step !== undefined) {
    params.set(DETAIL, view.step);
  }
  if (view.open.length > 0) {
    params.set(OPEN, view.open.join(","));
  }
  if (view.shut.length > 0) {
    params.set(SHUT, view.shut.join(","));
  }
  if (view.shown.length < KINDS.length) {
    params.set(KIND, KINDS.filter((kind) => view.shown.includes(kind)).join(","));
  }
  if (view.container !== undefined) {
    params.set(IN, view.container);
  }
  if (view.notRelevant || !view.undecided) {
    params.set(SHOW, [view.notRelevant ? "notrelevant" : "", view.undecided ? "conditional" : ""].filter(Boolean).join(","));
  }
  if (view.lens !== undefined) {
    params.set(LENS, view.lens);
  }
  if (view.origins) {
    params.set(ORIGINS, "1");
  }
  if (view.edit) {
    params.set(EDIT, "on");
  }
  if (view.select) {
    params.set(SELECT, "1");
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

/** C2: `view` with `kind` kept at full strength or faded (a route's canvas: drawn or left out). */
export function withKind(view: CanvasView, kind: NodeKind, shown: boolean): CanvasView {
  return { ...view, shown: KINDS.filter((each) => (each === kind ? shown : view.shown.includes(each))) };
}

/**
 * `view` opened on a node with `ancestors` (its containers, nearest first) and `kind`, with
 * whatever the view hides of it shown: its containers expanded, the least step that draws its
 * kind, its kind at full strength, and its class when it is not relevant or conditional; for a
 * node that is not a decision, the whole journey rather than the decisions alone. What the view
 * hides of other nodes stays.
 */
export function revealing(view: CanvasView, node: { kind: NodeKind; ancestors: readonly string[] }, state: string | undefined): CanvasView {
  const step = view.step ?? "stages";
  return {
    ...withKind(view, node.kind, true),
    step: kindsAt(step).includes(node.kind) ? view.step : STEPS.slice(STEPS.indexOf(step)).find((each) => kindsAt(each).includes(node.kind)),
    open: [...new Set([...view.open, ...node.ancestors])],
    shut: view.shut.filter((key) => !node.ancestors.includes(key)),
    notRelevant: view.notRelevant || state === "not_relevant",
    undecided: view.undecided || state === "conditional",
    rest: node.kind === "decision" ? view.rest : view.rest.filter(([name]) => name !== "decisions"),
  };
}

/** The view a fresh canvas opens with. */
export const DEFAULT_VIEW: CanvasView = { ...DEFAULT_SETTINGS, edit: false, select: false, map: false, rest: [] };

/** What a canvas is laid out for besides its graph: the step, the relevance shown. Expanding a container keeps it, so the layout keeps its positions (C15). */
export function layoutViewOf(view: CanvasSettings, step?: Step): string {
  return JSON.stringify([step ?? null, view.shown, view.container ?? null, view.notRelevant, view.undecided]);
}

/** The path of a journey's canvas (PLAN, GRAPH), or of a node's detail or an edge's card on it, keeping what the canvas shows. */
export function canvasPath(journey: string, view: CanvasView, node?: string): string {
  return `/journeys/${journey}/plan/graph${node === undefined ? "" : `/nodes/${node}`}${searchOf(view)}`;
}

/** An edge's card on the canvas: the ends it joins, `<from>~<to>`, in the address. */
export function edgePath(journey: string, view: CanvasView, edge: { from: string; to: string }): string {
  return `/journeys/${journey}/plan/graph/edges/${edge.from}~${edge.to}${searchOf(view)}`;
}

/** What the ladder asks of the level (5.1): the step, and the containers it and the viewer's clicks collapse. */
export interface Ladder {
  kinds: NodeKind[];
  collapsed: string[];
}

/**
 * The level request for what the canvas shows (C2). A journey names the kinds its step draws and
 * the containers collapsed, and the relevance classes to draw: what is left out rolls up and
 * re-targets its edges in the engine. The conditional class also holds the nodes waiting on an
 * undecided decision. A route's canvas, with no ladder, asks for the kinds shown and the container
 * drilled into.
 */
export function levelRequest(view: CanvasView, ladder?: Ladder): Extract<ProjectionRequest, { projection: "level" }> {
  const display: LevelDisplay[] = ["relevant", ...(view.undecided ? (["conditional"] as const) : []), ...(view.notRelevant ? (["not_relevant"] as const) : [])];
  return {
    projection: "level",
    shown: ladder === undefined ? view.shown : ladder.kinds,
    ...(view.container === undefined ? {} : { container: view.container }),
    ...(ladder === undefined || ladder.collapsed.length === 0 ? {} : { collapsed: ladder.collapsed }),
    ...(display.length === 3 ? {} : { display }),
  };
}
