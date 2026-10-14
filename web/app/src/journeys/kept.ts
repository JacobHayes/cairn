// What a journey's screens keep for when the viewer comes back to them: the settings in a
// screen's query (the graph's expanded containers and step, the list's sort and columns) and
// the list's open levels. Switching view leaves a screen, and returning must not be a fresh
// start. Held in memory for the tab's life, not stored: a query is an address, which is the
// shareable record, and a reload opens what its address says.
import { carriedSearch, type Screen } from "./address.ts";

/** The graph's modes, which a view of it is not: back on it, editing and selecting are off and the map is closed. */
const MODES = ["edit", "select", "map"];

const queries = new Map<string, string>();

/** Remembers `search` as where `journey`'s `screen` was left. */
export function keepQuery(journey: string, screen: Screen, search: string): void {
  const params = new URLSearchParams(search);
  MODES.forEach((mode) => { params.delete(mode); });
  queries.set(`${journey}/${screen}`, params.toString());
}

/** The query `screen` was last left with: what a return to it opens. */
export function keptQuery(journey: string, screen: Screen): string {
  return queries.get(`${journey}/${screen}`) ?? "";
}

/** The query of `to`'s address when the viewer goes there from `from`, which is open on `search`; the screen already open keeps its own. */
export function switchedSearch(journey: string, search: string, from: Screen, to: Screen): string {
  return from === to ? search : carriedSearch(search, from, to, keptQuery(journey, to));
}

/** What the viewer opened and folded in the list's tree, by container, and whether Open every level or Fold to the top level was the last word. */
export interface Folds {
  by: ReadonlyMap<string, boolean>;
  every: boolean | undefined;
}

const NONE: Folds = { by: new Map(), every: undefined };

const folded = new Map<string, Folds>();

/** `journey`'s list tree as it was left. */
export function keptFolds(journey: string): Folds {
  return folded.get(journey) ?? NONE;
}

export function keepFolds(journey: string, folds: Folds): void {
  folded.set(journey, folds);
}

/** Where a canvas was panned and zoomed to. */
export interface Viewport {
  x: number;
  y: number;
  zoom: number;
}

const viewports = new Map<string, Viewport>();

/** Remembers where the canvas `key` (a journey's, at one level) is looking. */
export function keepViewport(key: string, viewport: Viewport): void {
  viewports.set(key, viewport);
}

/** Where the canvas `key` was left, so opened again it looks there rather than fitting afresh. */
export function keptViewport(key: string): Viewport | undefined {
  return viewports.get(key);
}
