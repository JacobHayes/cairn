// What a canvas shows lives in its address, so a view is shareable, survives a reload, and
// follows the back button. A graph, a journey's or a route's, reads the page's view state
// (journeys/view.ts: the depth, the Filter and the search, shared with every view) and its own
// settings: the containers the viewer expanded and collapsed since picking the depth (`open=`,
// `shut=`, C4), the Signals lens (`lens=`, C6) and whether its structure is being edited (5.6). Every setting at its default leaves the
// address bare.
import { LENSES, type Lens } from "./model.ts";
import { journeyPath } from "../journeys/address.ts";
import { DEFAULT_VIEW_STATE, JOURNEY_OFFERS, VIEW_PARAMS, viewStateFrom, viewStateParams, type ViewOffers, type ViewState } from "../journeys/view.ts";

/** The containers the viewer expanded and collapsed since picking the depth (C4). */
const OPEN = "open";
const SHUT = "shut";
const LENS = "lens";
const EDIT = "edit";

/** The values of `name`, split on commas, empty when the parameter is there but holds none. */
function listOf(params: URLSearchParams, name: string): string[] | undefined {
  const value = params.get(name);
  return value === null ? undefined : value.split(",").filter(Boolean);
}

/** The lens `lens=` names; none when it names none. */
function lensFrom(params: URLSearchParams): Lens | undefined {
  const lens = params.get(LENS);
  return LENSES.find((each) => each === lens);
}

/** A graph's own settings: the viewer's expands and collapses, the lens and edit mode. */
export interface GraphSettings {
  open: string[];
  shut: string[];
  lens: Lens | undefined;
  edit: boolean;
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
  };
}

/** What a graph shows, from an address's query, the view state as far as its subject `offers` (a route offers less than a journey). */
export function graphViewFrom(params: URLSearchParams, offers: ViewOffers = JOURNEY_OFFERS): GraphView {
  return { ...graphSettingsFrom(params), state: viewStateFrom(params, offers) };
}

/** The view a journey's graph opens with: the stock view state, nothing expanded, no lens. */
export const DEFAULT_GRAPH_VIEW: GraphView = { state: DEFAULT_VIEW_STATE, open: [], shut: [], lens: undefined, edit: false };

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
  return params;
}

/**
 * `search` with the view state and the graph's own settings rewritten to `view`'s, every other
 * parameter kept after them: what a link to the graph keeps of the page it is on.
 */
export function graphSearch(view: GraphView, search: string): string {
  const rest = new URLSearchParams(search);
  for (const name of [...VIEW_PARAMS, OPEN, SHUT, LENS, EDIT]) {
    rest.delete(name);
  }
  const params = graphParamsOf(view);
  for (const [name, value] of rest) {
    params.append(name, value);
  }
  return params.toString();
}

/**
 * A journey's graph showing `view` at `target` (a node, an edge, or neither): the view state and
 * the graph's own settings rewritten in `search`, every other parameter kept, and `view=graph`
 * written only off its default (`journeyPath`). `search` is the query of the page the link is on;
 * links from anywhere else pass none.
 */
export function graphPath(journey: string, view: GraphView, target: { node?: string | undefined; edge?: string | undefined }, search: string): string {
  return journeyPath({ journey, ...target }, "graph", graphSearch(view, search));
}
