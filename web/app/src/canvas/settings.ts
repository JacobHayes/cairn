// What the canvas shows lives in its address, so a view is shareable, survives a reload, and
// follows the back button: the kinds hidden (C2), the container drilled into (C4), whether
// not-relevant and undecided nodes are hidden (C1), the heat overlay (C6), the trace of the
// open node (C7), and on a journey whether its structure is being edited (5.6). Every setting
// at its default leaves the address bare.
import type { NodeKind } from "../detail/model.ts";
import { DEFAULT_SETTINGS, KINDS, type CanvasSettings } from "./model.ts";

/** The canvas's settings, whether the open node is traced, and whether the structure is edited. */
export interface CanvasView extends CanvasSettings {
  trace: boolean;
  edit: boolean;
}

const HIDE = "hide";
const IN = "in";
const NOT_RELEVANT = "notrelevant";
const UNDECIDED = "undecided";
const HEAT = "heat";
const TRACE = "trace";
const EDIT = "edit";

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
export const DEFAULT_VIEW: CanvasView = { ...DEFAULT_SETTINGS, trace: false, edit: false };

/** What a layout depends on besides the graph: the kinds, the container, the relevance shown. */
export function layoutViewOf(view: CanvasSettings): string {
  return JSON.stringify([view.shown, view.container ?? null, view.notRelevant, view.undecided]);
}

/** The path of a journey's canvas, or of a node's detail on it, keeping what the canvas shows. */
export function canvasPath(journey: string, view: CanvasView, node?: string): string {
  return `/journeys/${journey}${node === undefined ? "" : `/nodes/${node}`}${searchOf(view)}`;
}
