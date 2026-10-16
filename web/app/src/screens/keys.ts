// The keyboard map (3.4): keys never fire while a field has focus, and `?` opens the sheet.
// The bindings that ship with the journey pages move you or change the view: `j` and `k`
// (next and previous row), `Enter` (the open node's primary action), `Esc` (close a popover,
// then the sheet, then clear the selection),
// `v` (next view), `F` (FRONTIER; the canvas has `f` for fit), `w` (the Walkthrough), `m` (Only mine),
// `1` to `4` (Depth's step), `/` (search), `?` (the sheet). The view-state keys change the journey's one view state
// (journeys/view.ts) wherever their control is live: none on the Summary, and not `F` on the Walkthrough, whose every card
// is on the frontier. `p` (pass) is the Walkthrough's own (acting/TriageScreen.tsx). Pure helpers carry the decisions, so
// they are tested without a page.
import { useEffect } from "react";
import { useLocation, useNavigate } from "react-router";

import { STEPS, type Step } from "../canvas/model.ts";
import { screenPath, nodePath } from "../detail/parts.tsx";
import { nextProjection, type Projection, type Screen } from "../journeys/address.ts";
import { viewPath, type ViewState } from "../journeys/view.ts";
import { typing } from "../ui/typing.ts";

/** The bindings the sheet lists, each with its words; `group` heads the run of rows it starts. The canvas handles its own keys (canvas/GraphCanvas.tsx) and the table lists them. */
export const KEY_BINDINGS: { keys: string; words: string; group?: string }[] = [
  { keys: "j / k", words: "Next or previous row" },
  { keys: "Enter", words: "The open node's primary action" },
  { keys: "Esc", words: "Close a popover or the sheet, then clear the selection" },
  { keys: "v", words: "Cycle Graph, List and Timeline" },
  { keys: "Shift F", words: "Toggle Frontier" },
  { keys: "w", words: "Open the Walkthrough" },
  { keys: "m", words: "Toggle Only mine" },
  { keys: "1 - 4", words: "Depth: Stages, Decisions, Deliverables or Actions" },
  { keys: "/", words: "Search" },
  { keys: "p", words: "Pass, in the Walkthrough" },
  { keys: "?", words: "This sheet" },
  { keys: "f", words: "Fit the selection, or the whole graph with none", group: "Graph" },
];

/** The step key `key` picks (`1` Stages to `4` Actions), when the journey offers it. */
export function stepForKey(key: string, steps: readonly Step[]): Step | undefined {
  const step = /^[1-4]$/.test(key) ? STEPS[Number(key) - 1] : undefined;
  return step !== undefined && steps.includes(step) ? step : undefined;
}

/** The key `step` rows after `selected` in `keys` (the first when none is selected), or none past an end. */
export function rowAfter(keys: readonly string[], selected: string | undefined, step: 1 | -1): string | undefined {
  if (keys.length === 0) {
    return undefined;
  }
  const at = selected === undefined ? -1 : keys.indexOf(selected);
  if (at === -1) {
    return step === 1 ? keys[0] : keys.at(-1);
  }
  return keys[at + step];
}

/** The keys of the rows the page lists now, in order. */
function visibleRows(): string[] {
  const rows = document.querySelectorAll<HTMLElement>('[data-testid="list-row"]');
  return [...rows].map((row) => row.dataset["node"] ?? "").filter(Boolean);
}

/** Presses the selected node's primary button in its inspector: its answer form, its Done. False when it has none. */
function pressPrimary(node: string): boolean {
  const button = document.querySelector<HTMLButtonElement>(`[data-testid="node-detail"][data-node="${node}"] button.primary:not(:disabled)`);
  button?.click();
  return button !== null;
}

/** Puts the focus in the toolbar's search; false when this page has none. */
function focusSearch(): boolean {
  const field = document.querySelector<HTMLElement>("[data-search-input]");
  field?.focus();
  return field !== null;
}

export interface KeysProps {
  journey: string;
  screen: Screen;
  selected: string | undefined;
  sheetOpen: boolean;
  setSheetOpen: (open: boolean) => void;
  /** The view state, the steps the journey offers, and how a key changes the state. */
  state: ViewState;
  steps: readonly Step[];
  onState: (next: ViewState, picked?: boolean) => void;
}

/** What one key press acts on: the props, and where the page is now. */
interface Heard extends KeysProps {
  pathname: string;
  search: string;
  go: (to: string) => void;
}

/** Escape closes a popover (which closes itself), then the sheet, then clears the selection. */
function escape({ selected, sheetOpen, setSheetOpen, pathname, search, go }: Heard): void {
  if (document.querySelector("[data-popover]") !== null) {
    return;
  }
  if (sheetOpen) {
    setSheetOpen(false);
  } else if (selected !== undefined) {
    go(`${screenPath(pathname)}${search}`);
  }
}

/** The view state's keys: where their control is live, each changes the state (the summary has none). */
function onViewState(key: string, { screen, state, steps, onState }: Heard): boolean {
  if (!["F", "m", "1", "2", "3", "4"].includes(key)) {
    return false;
  }
  if (screen === "summary") {
    return true;
  }
  const step = stepForKey(key, steps);
  if (step !== undefined) {
    onState({ ...state, depth: step }, !state.fade);
  } else if (key === "F" && screen !== "walk") {
    onState({ ...state, frontier: !state.frontier });
  } else if (key === "m") {
    onState({ ...state, filter: { ...state.filter, mine: !state.filter.mine } });
  }
  return true;
}

/** The views and the rows: keys that exist where a journey screen has them. */
function onChips(key: string, heard: Heard): boolean {
  const { journey, screen, selected, pathname, search, go } = heard;
  if (onViewState(key, heard)) {
    return true;
  }
  switch (key) {
    case "v": {
      if (screen !== "graph" && screen !== "list" && screen !== "timeline") {
        return false;
      }
      const to: Projection = nextProjection(screen);
      go(viewPath(journey, screen, to, selected, search));
      return true;
    }
    case "w":
      go(viewPath(journey, screen, "walk", selected, search));
      return true;
    case "/":
      return focusSearch();
    case "j":
    case "k": {
      const to = rowAfter(visibleRows(), selected, key === "j" ? 1 : -1);
      if (to !== undefined) {
        go(`${nodePath(screenPath(pathname), to)}${search}`);
      }
      return true;
    }
    default:
      return false;
  }
}

function heardKey(event: KeyboardEvent, heard: Heard): void {
  if (event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey || typing(event.target)) {
    return;
  }
  const { selected, sheetOpen, setSheetOpen } = heard;
  // The sheet is modal: behind it, only the keys that close it act.
  if (sheetOpen && event.key !== "Escape" && event.key !== "?") {
    return;
  }
  if (event.key === "Escape") {
    escape(heard);
  } else if (event.key === "?") {
    event.preventDefault();
    setSheetOpen(!sheetOpen);
  } else if (event.key === "Enter" && selected !== undefined && !(event.target instanceof Element && event.target.closest("a, button, summary"))) {
    if (pressPrimary(selected)) {
      event.preventDefault();
    }
  } else if (onChips(event.key, heard)) {
    event.preventDefault();
  }
}

export function useJourneyKeys(props: KeysProps): void {
  const { pathname, search } = useLocation();
  const navigate = useNavigate();
  const { journey, screen, selected, sheetOpen, setSheetOpen, state, steps, onState } = props;
  useEffect(() => {
    const heard = (event: KeyboardEvent) => {
      heardKey(event, { journey, screen, selected, sheetOpen, setSheetOpen, state, steps, onState, pathname, search, go: (to) => void navigate(to) });
    };
    document.addEventListener("keydown", heard);
    return () => {
      document.removeEventListener("keydown", heard);
    };
  }, [journey, screen, selected, sheetOpen, setSheetOpen, state, steps, onState, pathname, search, navigate]);
}
