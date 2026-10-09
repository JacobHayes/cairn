// What the canvas shows lives in its address, so a view is shareable, survives a reload, and
// follows the back button: the kinds shown (C2), the container opened (C4), whether
// not-relevant and conditional nodes are shown (C1), the gravity lens (C6), the trace of the
// open node (C7), on a journey whether its structure is being edited (5.6), and on a phone
// whether the full-screen map is open. The canvas
// lives at `/journeys/<id>/plan/graph`; every parameter it does not own (the toolbar's
// `decisions`, `mine`, `q`) rides along untouched, so a link made from the canvas keeps the
// chips. Every setting at its default leaves the address bare.
import type { ProjectionRequest } from "@cairn/wasm";

import type { NodeKind } from "../detail/model.ts";
import { DEFAULT_SETTINGS, KINDS, type CanvasSettings, type LevelDisplay } from "./model.ts";

/**
 * The canvas's settings, whether the open node is traced, whether the structure is edited, and
 * whether the full-screen map is open (below 720px, where the canvas is otherwise a preview).
 */
export interface CanvasView extends CanvasSettings {
  trace: boolean;
  edit: boolean;
  map: boolean;
  /** The address's other parameters, in order, kept as they were. */
  rest: [string, string][];
}

/** The kinds shown (C2). */
const KIND = "kind";
/** The container opened (C4). */
const OPEN = "open";
/** What is shown beyond the settled nodes: `notrelevant` and `conditional` (C1). */
const SHOW = "show";
const LENS = "lens";
const TRACE = "trace";
const EDIT = "edit";
/** The full-screen map's address parameter (MapFrame), so Back closes it. */
export const MAP = "map";

/** The parameters the canvas reads; every other one is left to the toolbar and the page. */
const OWN = new Set([KIND, OPEN, SHOW, LENS, TRACE, EDIT, MAP]);

/** The values of `name`, split on commas, empty when the parameter is there but holds none. */
function listOf(params: URLSearchParams, name: string): string[] | undefined {
  const value = params.get(name);
  return value === null ? undefined : value.split(",").filter(Boolean);
}

/** The earlier canvas's parameters that took other names or other values: see `graphQueryFromOld`. */
export const OLD_CANVAS_PARAMS = ["hide", "in", "notrelevant", "undecided", "heat"];

/**
 * An earlier canvas query as today's: `hide=` becomes the kinds left shown (`kind=`), `in=` the
 * container opened (`open=`), `notrelevant=hide` and `undecided=hide` what is shown (`show=`),
 * and `heat=on` the gravity lens. Every other parameter stays, `trace` and `edit` among them
 * (selecting a node does not trace it until the graph is redesigned).
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
  const notRelevant = params.get("notrelevant") !== "hide";
  const conditional = params.get("undecided") !== "hide";
  if (!notRelevant || !conditional) {
    next.set(SHOW, [notRelevant ? "notrelevant" : "", conditional ? "conditional" : ""].filter(Boolean).join(","));
  }
  if (params.get("heat") === "on") {
    next.set(LENS, "gravity");
  }
  for (const [name, value] of params) {
    if (!OLD_CANVAS_PARAMS.includes(name)) {
      next.append(name, value);
    }
  }
  return next;
}

/** The canvas's settings in an address's query. */
export function viewFrom(params: URLSearchParams): CanvasView {
  const kinds = listOf(params, KIND);
  const shown = listOf(params, SHOW);
  return {
    shown: kinds === undefined ? KINDS : KINDS.filter((kind) => kinds.includes(kind)),
    container: params.get(OPEN) ?? undefined,
    // Until the graph's own track flips the default, an address with no `show` shows both.
    notRelevant: shown === undefined || shown.includes("notrelevant"),
    undecided: shown === undefined || shown.includes("conditional"),
    heat: params.get(LENS) === "gravity",
    trace: params.get(TRACE) === "on",
    edit: params.get(EDIT) === "on",
    map: params.get(MAP) === "1",
    rest: [...params.entries()].filter(([name]) => !OWN.has(name)),
  };
}

/** `view` as an address's query, leaving out what is at its default. */
export function paramsOf(view: CanvasView): URLSearchParams {
  const params = new URLSearchParams();
  if (view.shown.length < KINDS.length) {
    params.set(KIND, KINDS.filter((kind) => view.shown.includes(kind)).join(","));
  }
  if (view.container !== undefined) {
    params.set(OPEN, view.container);
  }
  if (!view.notRelevant || !view.undecided) {
    params.set(SHOW, [view.notRelevant ? "notrelevant" : "", view.undecided ? "conditional" : ""].filter(Boolean).join(","));
  }
  if (view.heat) {
    params.set(LENS, "gravity");
  }
  if (view.trace) {
    params.set(TRACE, "on");
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

/** C2: `view` with `kind` shown or hidden. */
export function withKind(view: CanvasView, kind: NodeKind, shown: boolean): CanvasView {
  return { ...view, shown: KINDS.filter((each) => (each === kind ? shown : view.shown.includes(each))) };
}

/**
 * `view` opened on `node`, traced, with whatever the view hides of it shown: its kind, its
 * class when it is not relevant or conditional, and, for a node that is not a decision, the
 * whole journey rather than the decisions alone. What the view hides of other nodes stays.
 */
export function revealing(view: CanvasView, node: { kind: NodeKind; parent?: string | null | undefined }, state: string | undefined): CanvasView {
  return {
    ...withKind(view, node.kind, true),
    container: node.parent ?? undefined,
    trace: true,
    notRelevant: view.notRelevant || state === "not_relevant",
    undecided: view.undecided || state === "conditional",
    rest: node.kind === "decision" ? view.rest : view.rest.filter(([name]) => name !== "decisions"),
  };
}

/** The view a fresh canvas opens with. */
export const DEFAULT_VIEW: CanvasView = { ...DEFAULT_SETTINGS, trace: false, edit: false, map: false, rest: [] };

/** What a layout depends on besides the graph: the kinds, the container, the relevance shown. */
export function layoutViewOf(view: CanvasSettings): string {
  return JSON.stringify([view.shown, view.container ?? null, view.notRelevant, view.undecided]);
}

/** The path of a journey's canvas (PLAN, GRAPH), or of a node's detail on it, keeping what the canvas shows. */
export function canvasPath(journey: string, view: CanvasView, node?: string): string {
  return `/journeys/${journey}/plan/graph${node === undefined ? "" : `/nodes/${node}`}${searchOf(view)}`;
}

/**
 * The level request for what the canvas shows (C2): the toggles name the relevance classes the
 * engine hides, so what they hide rolls up and re-targets its edges there. The undecided toggle
 * is the conditional class, which also holds the nodes waiting on an undecided decision.
 */
export function levelRequest(view: CanvasView): Extract<ProjectionRequest, { projection: "level" }> {
  const display: LevelDisplay[] = ["relevant", ...(view.undecided ? (["conditional"] as const) : []), ...(view.notRelevant ? (["not_relevant"] as const) : [])];
  return {
    projection: "level",
    shown: view.shown,
    ...(view.container === undefined ? {} : { container: view.container }),
    ...(display.length === 3 ? {} : { display }),
  };
}
