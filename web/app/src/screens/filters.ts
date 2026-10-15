// The filters as data (2.3): how many are active, and each as a removable chip under the
// view's controls (`overdue ×`). The graph has none: its Filter badge counts what it hides. Each screen reads its own settings from the address
// (acting/address.ts); a chip is one setting taken off, as the address
// that results. `FRONTIER` and `DECISIONS` are the toolbar's own buttons, so their state is shown there and they
// make no chip here; `MINE` lives in the filter and does. Pure, so the unit tests check it
// without a page.
import { listFrom, listPath, triageFrom, triagePath, type ListFlag } from "../acting/address.ts";
import { pagePath, withMineFlipped, withParam, type Screen } from "../journeys/address.ts";
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
function timelineFilters(journey: string, search: string, node: string | undefined): ActiveFilter[] {
  const settings = listFrom(new URLSearchParams(search));
  const path = (name: string, value: string | undefined) => pagePath(journey, "timeline", node, withParam(search, name, value));
  const filters: ActiveFilter[] = [];
  if (settings.flags.includes("mine")) {
    filters.push({ id: "mine", label: "mine", without: pagePath(journey, "timeline", node, withMineFlipped(search)) });
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

/** The filters on `screen` at `search`, in the order the filter lists them. */
export function activeFilters(journey: string, screen: Screen, search: string, node?: string): ActiveFilter[] {
  const params = new URLSearchParams(search);
  const filters: ActiveFilter[] = [];
  if (screen === "walk") {
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
  } else if (screen === "list") {
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
  } else if (screen === "timeline") {
    filters.push(...timelineFilters(journey, search, node));
  }
  return filters;
}
