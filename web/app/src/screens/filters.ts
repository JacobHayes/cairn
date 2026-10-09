// The toolbar's filters as data (2.3): how many are active, and each as a removable chip
// under the toolbar (`overdue ×`). Each projection reads its own settings from the address
// (acting/address.ts, canvas/settings.ts); a chip is one setting taken off, as the address
// that results. `DECISIONS` is the toolbar's own button, so its state is shown there and it
// makes no chip here; `MINE` lives in the filter and does. Pure, so the unit tests check it
// without a page.
import { listFrom, listPath, nextFrom, nextPath, triageFrom, triagePath, type ListFlag } from "../acting/address.ts";
import { KINDS } from "../canvas/model.ts";
import { canvasPath, viewFrom } from "../canvas/settings.ts";
import { pagePath, withMineFlipped, withParam, type JourneyPage, type Projection } from "../journeys/address.ts";
import { stateWord } from "../status/words.ts";

/** One active filter: its words, and the address with it taken off. */
export interface ActiveFilter {
  id: string;
  label: string;
  /** The address of the same screen with this filter off (the open node stays open). */
  without: string;
}

const FLAG_WORDS: Partial<Record<ListFlag, string>> = {
  mine: "mine",
  unassigned: "unassigned",
  next_up: "next up",
  decisions_needed: "decisions needed",
  needs_breakdown: "needs breakdown",
  active: "active",
  blocked: "blocked",
  overdue: "overdue",
  stale: "stale",
  snoozed: "snoozed",
  snoozed_and_overdue: "snoozed and overdue",
  shortfall: "short of days",
};

/** The timeline's filters: the kinds it is narrowed to (none under DECISIONS) and the search. */
function timelineFilters(journey: string, page: JourneyPage, search: string, node: string | undefined): ActiveFilter[] {
  const settings = listFrom(new URLSearchParams(search));
  const path = (name: string, value: string | undefined) => pagePath(journey, page, "timeline", node, withParam(search, name, value));
  const filters: ActiveFilter[] = [];
  if (settings.flags.includes("mine")) {
    filters.push({ id: "mine", label: "mine", without: pagePath(journey, page, "timeline", node, withMineFlipped(search)) });
  }
  if (!settings.decisions) {
    for (const kind of settings.kinds) {
      const rest = settings.kinds.filter((each) => each !== kind);
      filters.push({ id: `kind-${kind}`, label: kind, without: path("kind", rest.length === 0 ? undefined : rest.join(",")) });
    }
  }
  if (settings.text !== "") {
    filters.push({ id: "text", label: `"${settings.text}"`, without: path("q", undefined) });
  }
  return filters;
}

/** The graph's filters: the kinds kept in focus (the rest fade), and conditional nodes turned off. */
function graphFilters(journey: string, params: URLSearchParams, node: string | undefined): ActiveFilter[] {
  const view = viewFrom(params);
  const filters: ActiveFilter[] = [];
  if (view.shown.length < KINDS.length) {
    filters.push({ id: "kinds", label: `only ${view.shown.join(", ") || "no kinds"}`, without: canvasPath(journey, { ...view, shown: KINDS }, node) });
  }
  if (!view.undecided) {
    filters.push({ id: "hide-conditional", label: "no conditional", without: canvasPath(journey, { ...view, undecided: true }, node) });
  }
  return filters;
}

/** The filters on `projection` of `page` at `search`, in the order the filter lists them. */
export function activeFilters(journey: string, page: JourneyPage, projection: Projection, search: string, node?: string): ActiveFilter[] {
  const params = new URLSearchParams(search);
  const filters: ActiveFilter[] = [];
  if (page === "next" && projection === "list") {
    const settings = nextFrom(params);
    const path = (changed: Partial<typeof settings>) => nextPath(journey, { ...settings, ...changed }, node);
    if (settings.mine) {
      filters.push({ id: "mine", label: "mine", without: path({ mine: false }) });
    }
    for (const kind of settings.kinds) {
      filters.push({ id: `kind-${kind}`, label: kind, without: path({ kinds: settings.kinds.filter((each) => each !== kind) }) });
    }
    for (const flag of settings.flags) {
      filters.push({ id: `flag-${flag}`, label: FLAG_WORDS[flag] ?? flag, without: path({ flags: settings.flags.filter((each) => each !== flag) }) });
    }
    if (settings.text !== "") {
      filters.push({ id: "text", label: `"${settings.text}"`, without: path({ text: "" }) });
    }
  } else if (page === "next") {
    const settings = triageFrom(params);
    const path = (changed: Partial<typeof settings>) => triagePath(journey, { ...settings, ...changed }, node);
    if (settings.mine) {
      filters.push({ id: "mine", label: "mine", without: path({ mine: false }) });
    }
    if (!settings.decisions) {
      for (const kind of settings.kinds) {
        filters.push({ id: `kind-${kind}`, label: kind, without: path({ kinds: settings.kinds.filter((each) => each !== kind) }) });
      }
    }
    for (const flag of settings.flags) {
      filters.push({ id: `flag-${flag}`, label: FLAG_WORDS[flag] ?? flag, without: path({ flags: settings.flags.filter((each) => each !== flag) }) });
    }
    if (settings.text !== "") {
      filters.push({ id: "text", label: `"${settings.text}"`, without: path({ text: "" }) });
    }
  } else if (projection === "list") {
    const settings = listFrom(params);
    const path = (changed: Partial<typeof settings>) => listPath(journey, { ...settings, ...changed }, node);
    for (const flag of settings.flags) {
      filters.push({ id: `flag-${flag}`, label: FLAG_WORDS[flag] ?? flag, without: path({ flags: settings.flags.filter((each) => each !== flag) }) });
    }
    if (settings.within !== undefined) {
      filters.push({ id: "within", label: "in a group", without: path({ within: undefined }) });
    }
    if (settings.owner !== undefined) {
      filters.push({ id: "owner", label: "owner", without: path({ owner: undefined }) });
    }
    for (const state of settings.states) {
      filters.push({ id: `state-${state}`, label: stateWord(state), without: path({ states: settings.states.filter((each) => each !== state) }) });
    }
    if (settings.notRelevant) {
      filters.push({ id: "not-relevant", label: "with not relevant", without: path({ notRelevant: false }) });
    }
    if (!settings.decisions) {
      for (const kind of settings.kinds) {
        filters.push({ id: `kind-${kind}`, label: kind, without: path({ kinds: settings.kinds.filter((each) => each !== kind) }) });
      }
    }
    if (settings.text !== "") {
      filters.push({ id: "text", label: `"${settings.text}"`, without: path({ text: "" }) });
    }
  } else if (projection === "timeline") {
    filters.push(...timelineFilters(journey, page, search, node));
  } else if (projection === "graph") {
    filters.push(...graphFilters(journey, params, node));
  }
  return filters;
}
