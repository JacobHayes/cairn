// C1, C2, C9, C10, C11: the settings every view of a journey or a route shares, as one value. Depth (the
// ladder's step, each named after the deepest kind it draws), the Filter (only mine, an owner, the
// flags, the two relevance classes) and the search choose what every view draws, through one
// engine level request (requests.ts); Frontier only emphasises, and so does Depth with Fade on
// (every kind is drawn, those below the depth dimmed). The value lives in the address, is parsed from it once
// (`viewStateFrom`), and every switch between views carries it whole (address.ts, `screenSwitched`): a view
// that cannot honour a setting keeps it, shown disabled, for the next view. Each view's own
// settings (the graph's open containers, the list's sort and columns, the timeline's range) ride
// along in the query too, so switching back restores them. Every setting at its default leaves the address bare. Pure but for the context,
// so the unit tests read the rules without a page.
import { createContext, useContext } from "react";

import type { ListFlag } from "../acting/address.ts";
import { TITLE_BYTES_MAX } from "@cairn/client";
import { overBytes } from "../authoring/limits.ts";
import { STEPS, type Step } from "../canvas/model.ts";
import { journeyPath } from "./address.ts";

/** C9: the flags the Filter offers, in the order it lists them. */
export const FILTER_FLAGS: readonly ListFlag[] = ["overdue", "stale", "unassigned", "shortfall", "snoozed"];

/** The flags' words, as the Filter's checks and the chips under the toolbar say them. */
export const FLAG_WORDS: Partial<Record<ListFlag, string>> = {
  overdue: "Overdue",
  stale: "Stale",
  unassigned: "Unassigned",
  shortfall: "Short of days",
  snoozed: "Snoozed",
};

/** C1, C9: the Filter: what a node must match to be drawn, and which relevance classes are drawn. */
export interface ViewFilter {
  /** Only the viewer's own nodes (E4). */
  mine: boolean;
  /** Only the nodes this entity owns. */
  owner: string | undefined;
  /** Only the nodes carrying every one of these flags (`FILTER_FLAGS`, in its order). */
  flags: ListFlag[];
  /** Settled not-relevant nodes are drawn (hidden by default, C1). */
  notRelevant: boolean;
  /** Conditional nodes are drawn (shown by default, Q3). */
  conditional: boolean;
}

/** What every journey view shares. */
export interface ViewState {
  /** Depth's step (C2); none is the default, Actions. */
  depth: Step | undefined;
  /** Depth's Fade (C2): draw every kind and dim those below the depth instead of leaving them out. */
  fade: boolean;
  /** The Frontier highlight (C10). */
  frontier: boolean;
  filter: ViewFilter;
  /** The search (C9), as typed. */
  text: string;
}

/**
 * What a subject of the views offers of the shared state. A journey offers all of it; a route
 * has no state, owner or relevance to decide, so it offers Depth (with Fade), the conditional
 * class and the search, and the rest is neither shown nor read from its address.
 */
export interface ViewOffers {
  frontier: boolean;
  mine: boolean;
  owner: boolean;
  flags: boolean;
  notRelevant: boolean;
}

export const JOURNEY_OFFERS: ViewOffers = { frontier: true, mine: true, owner: true, flags: true, notRelevant: true };

export const ROUTE_OFFERS: ViewOffers = { frontier: false, mine: false, owner: false, flags: false, notRelevant: false };

export const DEFAULT_FILTER: ViewFilter = { mine: false, owner: undefined, flags: [], notRelevant: false, conditional: true };

export const DEFAULT_VIEW_STATE: ViewState = { depth: undefined, fade: false, frontier: false, filter: DEFAULT_FILTER, text: "" };

/** The address parameters the view state is written to, in the order they are written. */
export const VIEW_PARAMS = ["depth", "fade", "frontier", "mine", "owner", "flag", "show", "q"] as const;

/** The step `depth=` names; none when it names none, or names the default, Actions. */
export function stepFrom(params: URLSearchParams): Step | undefined {
  const step = params.get("depth");
  return STEPS.find((each) => each === step && each !== "actions");
}

/** The search an address names, or none when it is longer than the engine reads one (a title, PRACTICES, Explicit limits). */
function searchFrom(text: string): string {
  return overBytes(text.trim(), TITLE_BYTES_MAX, "The search") === undefined ? text : "";
}

/**
 * The view state in an address's query, less what the subject does not `offer`. `show=` absent
 * draws the conditional class and hides the not-relevant one; present, it lists the classes
 * drawn beyond the relevant one.
 */
export function viewStateFrom(params: URLSearchParams, offers: ViewOffers = JOURNEY_OFFERS): ViewState {
  const flags = new Set((params.get("flag") ?? "").split(","));
  const show = params.get("show");
  const shown = show === null ? undefined : show.split(",");
  const owner = params.get("owner");
  return {
    depth: stepFrom(params),
    fade: params.get("fade") === "1",
    frontier: offers.frontier && params.get("frontier") === "1",
    filter: {
      mine: offers.mine && params.get("mine") === "1",
      owner: !offers.owner || owner === null || owner === "" ? undefined : owner,
      flags: offers.flags ? FILTER_FLAGS.filter((flag) => flags.has(flag)) : [],
      notRelevant: offers.notRelevant && (shown?.includes("notrelevant") ?? DEFAULT_FILTER.notRelevant),
      conditional: shown === undefined ? DEFAULT_FILTER.conditional : shown.includes("conditional"),
    },
    text: searchFrom(params.get("q") ?? ""),
  };
}

/** `state` as query parameters, in `VIEW_PARAMS` order, leaving out what is at its default. */
export function viewStateParams(state: ViewState): URLSearchParams {
  const params = new URLSearchParams();
  const { filter } = state;
  if (state.depth !== undefined && state.depth !== "actions") {
    params.set("depth", state.depth);
  }
  if (state.fade) {
    params.set("fade", "1");
  }
  if (state.frontier) {
    params.set("frontier", "1");
  }
  if (filter.mine) {
    params.set("mine", "1");
  }
  if (filter.owner !== undefined) {
    params.set("owner", filter.owner);
  }
  const flags = FILTER_FLAGS.filter((flag) => filter.flags.includes(flag));
  if (flags.length > 0) {
    params.set("flag", flags.join(","));
  }
  if (filter.notRelevant || !filter.conditional) {
    params.set("show", [filter.notRelevant ? "notrelevant" : "", filter.conditional ? "conditional" : ""].filter(Boolean).join(","));
  }
  if (state.text !== "") {
    params.set("q", state.text);
  }
  return params;
}

/** The journey's list, filtered to `filter` (the stock filter with these settings changed): where a count of what needs a look leads. */
export function listPath(journey: string, filter: Partial<ViewFilter>): string {
  return journeyPath({ journey }, "list", viewStateParams({ ...DEFAULT_VIEW_STATE, filter: { ...DEFAULT_FILTER, ...filter } }).toString());
}

function searchOf(params: URLSearchParams): string {
  const text = params.toString();
  return text === "" ? "" : `?${text}`;
}

/**
 * `search` with its view state replaced by `next`, every other parameter kept in its order. The
 * viewer's expands and collapses (`open=`, `shut=`) hold until a depth is picked (5.1): they are
 * dropped when `picked` says one was picked, or when the depth changes with Fade off (with Fade
 * on every depth draws the one layout, which a step only dims, so the folds stay).
 */
export function withViewState(search: string, next: ViewState, picked = false): string {
  const now = new URLSearchParams(search);
  const params = viewStateParams(next);
  const restart = picked || (!next.fade && (stepFrom(now) ?? "actions") !== (next.depth ?? "actions"));
  for (const [name, value] of now) {
    if (!(VIEW_PARAMS as readonly string[]).includes(name) && !(restart && (name === "open" || name === "shut"))) {
      params.append(name, value);
    }
  }
  return searchOf(params);
}

/** Whether `filter` differs from the stock filter: the Filter's pressed look. */
export function filterChanged(filter: ViewFilter): boolean {
  return filter.mine || filter.owner !== undefined || filter.flags.length > 0 || filter.notRelevant !== DEFAULT_FILTER.notRelevant || filter.conditional !== DEFAULT_FILTER.conditional;
}

/** Whether `state` is the stock view: every setting but Frontier and Fade, which hide nothing, at its default. */
export function isStock(state: ViewState): boolean {
  return (state.depth ?? "actions") === "actions" && !filterChanged(state.filter) && state.text.trim() === "";
}

/** Reset the view: back to Actions depth, the stock filter and no search; Frontier and Fade are kept (neither hides anything). */
export function stockView(state: ViewState): ViewState {
  return { ...DEFAULT_VIEW_STATE, frontier: state.frontier, fade: state.fade };
}

/** One setting of the Filter as a removable chip: its words and the state without it. */
export interface FilterChip {
  id: string;
  label: string;
  without: ViewState;
}

/** C9: the Filter's settings off stock, each as a chip that takes it off, in the Filter's order. The search is not one: its box shows the text on every view, and clears it. */
export function filterChips(state: ViewState, ownerName: (key: string) => string): FilterChip[] {
  const { filter } = state;
  const withFilter = (changed: Partial<ViewFilter>): ViewState => ({ ...state, filter: { ...filter, ...changed } });
  const chips: FilterChip[] = [];
  if (filter.mine) {
    chips.push({ id: "mine", label: "Mine", without: withFilter({ mine: false }) });
  }
  if (filter.owner !== undefined) {
    chips.push({ id: "owner", label: `Owner: ${ownerName(filter.owner)}`, without: withFilter({ owner: undefined }) });
  }
  for (const flag of filter.flags) {
    chips.push({ id: `flag-${flag}`, label: FLAG_WORDS[flag] ?? flag, without: withFilter({ flags: filter.flags.filter((each) => each !== flag) }) });
  }
  if (filter.notRelevant) {
    chips.push({ id: "not-relevant", label: "With not relevant", without: withFilter({ notRelevant: false }) });
  }
  if (!filter.conditional) {
    chips.push({ id: "no-conditional", label: "Without conditional", without: withFilter({ conditional: true }) });
  }
  return chips;
}

/** The page's view state, and how a control changes it (`picked`: a depth was picked). */
export interface ViewStateValue {
  state: ViewState;
  onState: (next: ViewState, picked?: boolean) => void;
  /** How many times a depth has been picked or the view reset: the local folds of a list and a timeline start over with each. */
  picks: number;
}

export const ViewStateContext = createContext<ViewStateValue | undefined>(undefined);

/** The page's view state (screens/JourneyFrame.tsx and screens/RoutePage.tsx provide it). */
export function useViewState(): ViewStateValue {
  const value = useContext(ViewStateContext);
  if (value === undefined) {
    throw new Error("useViewState outside a view's page");
  }
  return value;
}
