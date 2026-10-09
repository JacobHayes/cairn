// The keyboard map (3.4): keys never fire while a field has focus, and `?` opens the sheet.
// The bindings that ship with the journey pages move you or change the view: `j` and `k`
// (next and previous row), `Enter` (the open node's primary action), `Esc` (close a popover,
// then the sheet, then clear the selection),
// `v` (next projection), `d` and `m` (DECISIONS and MINE), `/` (search), `?` (the sheet). `p`
// (pass) is the cards' own (acting/TriageScreen.tsx). Pure helpers carry the decisions, so they
// are tested without a page.
import { useEffect } from "react";
import { useLocation, useNavigate } from "react-router";

import { screenPath, nodePath } from "../detail/parts.tsx";
import { carriedSearch, chipApplies, nextProjection, pagePath, withMineFlipped, withParam, type JourneyPage, type Projection } from "../journeys/address.ts";
import { typing } from "../ui/typing.ts";

/** The bindings the sheet lists, each with its words. */
export const KEY_BINDINGS: { keys: string; words: string }[] = [
  { keys: "j / k", words: "Next or previous row" },
  { keys: "Enter", words: "The open node's primary action" },
  { keys: "Esc", words: "Close a popover or the sheet, then clear the selection" },
  { keys: "v", words: "Next projection" },
  { keys: "d", words: "Toggle Decisions" },
  { keys: "m", words: "Toggle Mine, where the projection has it" },
  { keys: "/", words: "Search or find" },
  { keys: "p", words: "Pass, on the cards" },
  { keys: "?", words: "This sheet" },
];

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
  const rows = document.querySelectorAll<HTMLElement>('[data-testid="next-item"], [data-testid="list-row"]');
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
  page: JourneyPage | "summary";
  projection: Projection | undefined;
  selected: string | undefined;
  sheetOpen: boolean;
  setSheetOpen: (open: boolean) => void;
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

/** The chips and the switcher: keys that exist where a journey page has a projection. */
function onProjection(key: string, heard: Heard, page: JourneyPage, projection: Projection): boolean {
  const { journey, selected, pathname, search, go } = heard;
  switch (key) {
    case "v": {
      const to = nextProjection(page, projection);
      go(pagePath(journey, page, to, selected, carriedSearch(search, page, to)));
      return true;
    }
    case "d":
      if (chipApplies("decisions", page, projection)) {
        go(`${pathname}${withParam(search, "decisions", new URLSearchParams(search).get("decisions") === "1" ? undefined : "1")}`);
      }
      return true;
    case "m":
      if (chipApplies("mine", page, projection)) {
        go(`${pathname}${withMineFlipped(search)}`);
      }
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
  const { page, projection, selected, sheetOpen, setSheetOpen } = heard;
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
  } else if (page !== "summary" && projection !== undefined && onProjection(event.key, heard, page, projection)) {
    event.preventDefault();
  }
}

export function useJourneyKeys(props: KeysProps): void {
  const { pathname, search } = useLocation();
  const navigate = useNavigate();
  const { journey, page, projection, selected, sheetOpen, setSheetOpen } = props;
  useEffect(() => {
    const heard = (event: KeyboardEvent) => {
      heardKey(event, { journey, page, projection, selected, sheetOpen, setSheetOpen, pathname, search, go: (to) => void navigate(to) });
    };
    document.addEventListener("keydown", heard);
    return () => {
      document.removeEventListener("keydown", heard);
    };
  }, [journey, page, projection, selected, sheetOpen, setSheetOpen, pathname, search, navigate]);
}
