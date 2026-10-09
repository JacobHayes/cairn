// What the canvas shows lives in its address, so a view is shareable, survives a reload, and
// follows the back button: the kinds hidden (C2), the container drilled into (C4), whether
// not-relevant and undecided nodes are hidden (C1), the heat overlay (C6), the trace of the
// open node (C7), on a journey whether its structure is being edited (5.6), and on a phone
// whether the full-screen map is open. Every setting
// at its default leaves the address bare.
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
}

const HIDE = "hide";
const IN = "in";
const NOT_RELEVANT = "notrelevant";
const UNDECIDED = "undecided";
const HEAT = "heat";
const TRACE = "trace";
const EDIT = "edit";
/** The full-screen map's address parameter (MapFrame), so Back closes it. */
export const MAP = "map";

/** The canvas's settings in an address's query. */
export function viewFrom(params: URLSearchParams): CanvasView {
  const hidden = new Set((params.get(HIDE) ?? "").split(",").filter(Boolean));
  return {
    shown: KINDS.filter((kind) => !hidden.has(kind)),
    container: params.get(IN) ?? undefined,
    notRelevant: params.get(NOT_RELEVANT) !== "hide",
    undecided: params.get(UNDECIDED) !== "hide",
    heat: params.get(HEAT) === "on",
    trace: params.get(TRACE) === "on",
    edit: params.get(EDIT) === "on",
    map: params.get(MAP) === "1",
  };
}

/** `view` as an address's query, leaving out what is at its default. */
export function paramsOf(view: CanvasView): URLSearchParams {
  const params = new URLSearchParams();
  const hidden = KINDS.filter((kind) => !view.shown.includes(kind));
  if (hidden.length > 0) {
    params.set(HIDE, hidden.join(","));
  }
  if (view.container !== undefined) {
    params.set(IN, view.container);
  }
  if (!view.notRelevant) {
    params.set(NOT_RELEVANT, "hide");
  }
  if (!view.undecided) {
    params.set(UNDECIDED, "hide");
  }
  if (view.heat) {
    params.set(HEAT, "on");
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

/** The view a fresh canvas opens with. */
export const DEFAULT_VIEW: CanvasView = { ...DEFAULT_SETTINGS, trace: false, edit: false, map: false };

/** What a layout depends on besides the graph: the kinds, the container, the relevance shown. */
export function layoutViewOf(view: CanvasSettings): string {
  return JSON.stringify([view.shown, view.container ?? null, view.notRelevant, view.undecided]);
}

/** The path of a journey's canvas, or of a node's detail on it, keeping what the canvas shows. */
export function canvasPath(journey: string, view: CanvasView, node?: string): string {
  return `/journeys/${journey}${node === undefined ? "" : `/nodes/${node}`}${searchOf(view)}`;
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
