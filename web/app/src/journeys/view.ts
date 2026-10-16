// C1, C2, C9, C10, C11: the settings every view of a journey shares, as one value. Detail (the
// ladder's step), the Filter (only mine, an owner, the flags, the two relevance classes) and the
// search choose what every view draws, through one engine level request (requests.ts); Frontier
// and Decisions never change the level (on the list and the walk through, Decisions also narrows the rows). The value lives in the address, is parsed from it once
// (`viewStateFrom`), and every switch between views carries it whole (`switchedSearch`): a view
// that cannot honour a setting keeps it, shown disabled, for the next view. Each view's own
// settings (the graph's open containers, the list's sort and columns, the timeline's range) stay
// that view's. Every setting at its default leaves the address bare. Pure but for the context,
// so the unit tests read the rules without a page.
import { createContext, useContext } from "react";

import type { ListFlag } from "../acting/address.ts";
import { overBytes, TITLE_BYTES_MAX } from "../authoring/limits.ts";
import { STEPS, type Step } from "../canvas/model.ts";
import type { Screen } from "./address.ts";

/** C9: the flags the Filter offers, in the order it lists them. */
export const FILTER_FLAGS: readonly ListFlag[] = ["overdue", "stale", "unassigned", "shortfall", "snoozed"];

/** The flags' words, as the chips under the toolbar say them. */
export const FLAG_WORDS: Partial<Record<ListFlag, string>> = {
  mine: "mine",
  overdue: "overdue",
  stale: "stale",
  unassigned: "unassigned",
  shortfall: "short of days",
  snoozed: "snoozed",
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
  /** The detail step (C2); none is the default, All. */
  step: Step | undefined;
  /** The Frontier highlight (C10). */
  frontier: boolean;
  /** The Decisions chip (C11, C12). */
  decisions: boolean;
  filter: ViewFilter;
  /** The search (C9), as typed. */
  text: string;
}

export const DEFAULT_FILTER: ViewFilter = { mine: false, owner: undefined, flags: [], notRelevant: false, conditional: true };

export const DEFAULT_VIEW_STATE: ViewState = { step: undefined, frontier: false, decisions: false, filter: DEFAULT_FILTER, text: "" };

/** The address parameters the view state is written to, in the order they are written. */
export const VIEW_PARAMS = ["detail", "frontier", "decisions", "mine", "owner", "flag", "show", "q"] as const;

/** The step `detail=` names; none when it names none, or names the default, All. */
export function stepFrom(params: URLSearchParams): Step | undefined {
  const step = params.get("detail");
  return STEPS.find((each) => each === step && each !== "all");
}

/** The search an address names, or none when it is longer than the engine reads one (a title, PRACTICES, Explicit limits). */
function searchFrom(text: string): string {
  return overBytes(text.trim(), TITLE_BYTES_MAX, "The search") === undefined ? text : "";
}

/**
 * The view state in an address's query. `show=` absent draws the conditional class and hides the
 * not-relevant one; present, it lists the classes drawn beyond the relevant one.
 */
export function viewStateFrom(params: URLSearchParams): ViewState {
  const flags = new Set((params.get("flag") ?? "").split(","));
  const show = params.get("show");
  const shown = show === null ? undefined : show.split(",");
  const owner = params.get("owner");
  return {
    step: stepFrom(params),
    frontier: params.get("frontier") === "1",
    decisions: params.get("decisions") === "1",
    filter: {
      mine: params.get("mine") === "1",
      owner: owner === null || owner === "" ? undefined : owner,
      flags: FILTER_FLAGS.filter((flag) => flags.has(flag)),
      notRelevant: shown?.includes("notrelevant") ?? DEFAULT_FILTER.notRelevant,
      conditional: shown === undefined ? DEFAULT_FILTER.conditional : shown.includes("conditional"),
    },
    text: searchFrom(params.get("q") ?? ""),
  };
}

/** `state` as query parameters, in `VIEW_PARAMS` order, leaving out what is at its default. */
export function viewStateParams(state: ViewState): URLSearchParams {
  const params = new URLSearchParams();
  const { filter } = state;
  if (state.step !== undefined && state.step !== "all") {
    params.set("detail", state.step);
  }
  if (state.frontier) {
    params.set("frontier", "1");
  }
  if (state.decisions) {
    params.set("decisions", "1");
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

function searchOf(params: URLSearchParams): string {
  const text = params.toString();
  return text === "" ? "" : `?${text}`;
}

/**
 * `search` with its view state replaced by `next`, every other parameter kept in its order. The
 * viewer's expands and collapses (`open=`, `shut=`) hold until a step is picked (5.1): they are
 * dropped when the step changes or when `picked` says one was picked.
 */
export function withViewState(search: string, next: ViewState, picked = false): string {
  const now = new URLSearchParams(search);
  const params = viewStateParams(next);
  const restart = picked || (stepFrom(now) ?? "all") !== (next.step ?? "all");
  for (const [name, value] of now) {
    if (!(VIEW_PARAMS as readonly string[]).includes(name) && !(restart && (name === "open" || name === "shut"))) {
      params.append(name, value);
    }
  }
  return searchOf(params);
}

/**
 * The query a switch from `from` to `to` opens: the whole view state, and nothing of `from`'s
 * own settings. The screen already open keeps its query.
 */
export function switchedSearch(search: string, from: Screen, to: Screen): string {
  return from === to ? search : searchOf(viewStateParams(viewStateFrom(new URLSearchParams(search))));
}

/** Whether `filter` differs from the stock filter: the Filter's pressed look. */
export function filterChanged(filter: ViewFilter): boolean {
  return filter.mine || filter.owner !== undefined || filter.flags.length > 0 || filter.notRelevant !== DEFAULT_FILTER.notRelevant || filter.conditional !== DEFAULT_FILTER.conditional;
}

/** Whether `state` is the stock view: every setting but Frontier, which hides nothing, at its default. */
export function isStock(state: ViewState): boolean {
  return (state.step ?? "all") === "all" && !state.decisions && !filterChanged(state.filter) && state.text.trim() === "";
}

/** Reset the view: back to All, the stock filter, no search and Decisions off; Frontier is kept. */
export function stockView(state: ViewState): ViewState {
  return { ...DEFAULT_VIEW_STATE, frontier: state.frontier };
}

/** One setting of the Filter or the search as a removable chip: its words and the state without it. */
export interface FilterChip {
  id: string;
  label: string;
  without: ViewState;
}

/** C9: the Filter's settings off stock and the search, each as a chip that takes it off, in the Filter's order. */
export function filterChips(state: ViewState, ownerName: (key: string) => string): FilterChip[] {
  const { filter } = state;
  const withFilter = (changed: Partial<ViewFilter>): ViewState => ({ ...state, filter: { ...filter, ...changed } });
  const chips: FilterChip[] = [];
  if (filter.mine) {
    chips.push({ id: "mine", label: "mine", without: withFilter({ mine: false }) });
  }
  if (filter.owner !== undefined) {
    chips.push({ id: "owner", label: `owner: ${ownerName(filter.owner)}`, without: withFilter({ owner: undefined }) });
  }
  for (const flag of filter.flags) {
    chips.push({ id: `flag-${flag}`, label: FLAG_WORDS[flag] ?? flag, without: withFilter({ flags: filter.flags.filter((each) => each !== flag) }) });
  }
  if (filter.notRelevant) {
    chips.push({ id: "not-relevant", label: "with not relevant", without: withFilter({ notRelevant: false }) });
  }
  if (!filter.conditional) {
    chips.push({ id: "no-conditional", label: "without conditional", without: withFilter({ conditional: true }) });
  }
  if (state.text !== "") {
    chips.push({ id: "text", label: `"${state.text}"`, without: { ...state, text: "" } });
  }
  return chips;
}

/** The journey page's view state, and how a control changes it (`picked`: a step was picked). */
export interface ViewStateValue {
  state: ViewState;
  onState: (next: ViewState, picked?: boolean) => void;
  /** How many times a step has been picked or the view reset: the local folds of a list and a timeline start over with each. */
  picks: number;
}

export const ViewStateContext = createContext<ViewStateValue | undefined>(undefined);

/** The journey page's view state (screens/JourneyFrame.tsx provides it). */
export function useViewState(): ViewStateValue {
  const value = useContext(ViewStateContext);
  if (value === undefined) {
    throw new Error("useViewState outside a journey page");
  }
  return value;
}
