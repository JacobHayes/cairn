// The keyboard map: keys never fire while a field has focus, and `?` opens the sheet.
// The bindings that ship with the journey pages (and the route's, for what a route offers) move you or change the view: `j` and `k`
// (next and previous row), `Enter` (the open node's primary action), `Esc` (close a popover,
// then the sheet, then clear the selection),
// `v` (next view), `F` (FRONTIER; the canvas has `f` for fit), `w` (the Walkthrough), `m` (Only mine),
// `1` to `4` (Depth's step), `/` (search), `?` (the sheet). The view-state keys change the journey's one view state
// (journeys/view.ts) wherever their control is live, as the per-view table says (`SCREEN_VIEWS`: none on the Summary,
// and not `F` on the Walkthrough), and only what the page's subject offers (`ViewOffers`) is keyed or listed on the
// sheet. `p` (pass) is the Walkthrough's own (acting/TriageScreen.tsx). Pure helpers carry the decisions, so
// they are tested without a page.
import { useEffect, useMemo } from "react";
import { useLocation, useNavigate } from "react-router";

import { STEPS, type Step } from "../canvas/model.ts";
import { nextProjection, PROJECTIONS, screenSwitched, unavailableScreens, useNodeAddresses, type NodeAddresses, type Screen } from "../journeys/address.ts";
import { useJourneyTarget } from "../journeys/useJourneyTarget.ts";
import { JOURNEY_OFFERS, SCREEN_VIEWS, type ViewOffers, type ViewState } from "../journeys/view.ts";
import { overlayOpen, typing } from "../ui/typing.ts";

/** One binding the sheet lists, with its words; `group` heads the run of rows it starts, and `needs` is what the page must offer for it to act (`keyBindingsFor`). */
export interface KeyBinding {
  keys: string;
  words: string;
  group?: string;
  needs?: "frontier" | "mine" | "walk" | "rows" | "view" | "acts";
}

/** What a page offers its keys beyond its subject's offers: the screen shown, the views it cannot show now, and whether its nodes can be acted on. Left out, each is taken as offered (a sheet for no page in particular). */
export interface KeyPage {
  screen?: Screen | undefined;
  unavailable?: Partial<Record<Screen, string>>;
  acts?: boolean;
}

/** Every binding, in the sheet's order. The canvas handles its own keys (canvas/GraphCanvas.tsx) and the table lists them. */
const KEY_BINDINGS: readonly KeyBinding[] = [
  { keys: "j / k", words: "Next or previous row", needs: "rows" },
  { keys: "Enter", words: "The open node's primary action", needs: "acts" },
  { keys: "Esc", words: "Close a popover or the sheet, then clear the selection" },
  { keys: "v", words: "Cycle Graph, List and Timeline" },
  { keys: "Shift F", words: "Toggle Frontier", needs: "frontier" },
  { keys: "w", words: "Open the Walkthrough", needs: "walk" },
  { keys: "m", words: "Toggle Only mine", needs: "mine" },
  { keys: "1 - 4", words: "Depth: Stages, Decisions, Deliverables or Actions", needs: "view" },
  { keys: "/", words: "Search", needs: "view" },
  { keys: "p", words: "Pass, in the Walkthrough", needs: "walk" },
  { keys: "]", words: "Hide or show the inspector" },
  { keys: "?", words: "This sheet" },
  { keys: "f", words: "Fit the selection, or the whole graph with none", group: "Graph" },
];

/** Whether the page's subject has the Walkthrough, and it can be shown now. */
function walks(offers: ViewOffers, unavailable: Partial<Record<Screen, string>>): boolean {
  return offers.screens.includes("walk") && unavailable.walk === undefined;
}

/** Whether `binding` acts on `page`, whose subject `offers` this; the screen's part is the per-screen table's (`SCREEN_VIEWS`). */
function keyActs(binding: KeyBinding, offers: ViewOffers, page: KeyPage): boolean {
  const view = page.screen === undefined ? undefined : SCREEN_VIEWS[page.screen];
  const viewState = view?.viewState !== false;
  switch (binding.needs) {
    case undefined:
      return true;
    case "walk":
      return walks(offers, page.unavailable ?? {});
    case "rows":
      return view?.rows !== false;
    case "view":
      return viewState;
    case "acts":
      return page.acts !== false;
    case "frontier":
      return offers.frontier && viewState && view?.frontier === undefined;
    default:
      return offers.mine && viewState;
  }
}

/** The bindings that act on `page`, whose subject `offers` this: what its sheet lists. */
export function keyBindingsFor(offers: ViewOffers, page: KeyPage = {}): KeyBinding[] {
  return KEY_BINDINGS.filter((binding) => keyActs(binding, offers, page));
}

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
  const rows = document.querySelectorAll<HTMLElement>('[data-testid="list-row"], [data-testid="route-row"][data-navigable]');
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
  screen: Screen;
  selected: string | undefined;
  sheetOpen: boolean;
  setSheetOpen: (open: boolean) => void;
  /** The view state, the steps the journey offers, and how a key changes the state. */
  state: ViewState;
  steps: readonly Step[];
  onState: (next: ViewState, picked?: boolean) => void;
  /** What the page's subject offers: `m`, `F` and `w` act only where it offers Only mine, Frontier and the Walkthrough. */
  offers: ViewOffers;
  /** The views the page lists but cannot show now, whose keys do nothing. */
  unavailable: Partial<Record<Screen, string>>;
  /** The address of view `to` from the page as it is now. */
  viewAddress: (to: Screen) => string;
}

/** What one key press acts on: the props, and where the page is now. */
interface Heard extends KeysProps {
  addresses: NodeAddresses;
  go: (to: string) => void;
}

/** Escape closes a popover (which closes itself), then the sheet, then clears the selection. */
function escape({ selected, sheetOpen, setSheetOpen, addresses, go }: Heard): void {
  if (document.querySelector("[data-popover]") !== null) {
    return;
  }
  if (sheetOpen) {
    setSheetOpen(false);
  } else if (selected !== undefined) {
    go(addresses.close());
  }
}

/** The view state's keys: where their control is live (`SCREEN_VIEWS`), each changes the state. */
function onViewState(key: string, { screen, state, steps, onState, offers }: Heard): boolean {
  if (!["F", "m", "1", "2", "3", "4"].includes(key) || (key === "F" && !offers.frontier) || (key === "m" && !offers.mine)) {
    return false;
  }
  const view = SCREEN_VIEWS[screen];
  if (!view.viewState) {
    return true;
  }
  const step = stepForKey(key, steps);
  if (step !== undefined) {
    onState({ ...state, depth: step }, !state.fade);
  } else if (key === "F" && view.frontier === undefined) {
    onState({ ...state, frontier: !state.frontier });
  } else if (key === "m") {
    onState({ ...state, filter: { ...state.filter, mine: !state.filter.mine } });
  }
  return true;
}

/** The views and the rows: keys that exist where a journey screen has them. */
function onChips(key: string, heard: Heard): boolean {
  const { screen, selected, addresses, viewAddress, offers, unavailable, go } = heard;
  if (onViewState(key, heard)) {
    return true;
  }
  switch (key) {
    case "v": {
      const projection = PROJECTIONS.find((each) => each === screen);
      if (projection === undefined) {
        return false;
      }
      go(viewAddress(nextProjection(projection)));
      return true;
    }
    case "w":
      if (!walks(offers, unavailable)) {
        return false;
      }
      go(viewAddress("walk"));
      return true;
    case "/":
      return focusSearch();
    case "j":
    case "k": {
      const to = rowAfter(visibleRows(), selected, key === "j" ? 1 : -1);
      if (to !== undefined) {
        go(addresses.open(to));
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
  // Behind an open popover, menu or the tour's callout, nothing but that layer hears a key (the key sheet, itself a dialog, takes its own keys below).
  if (overlayOpen() && !sheetOpen) {
    return;
  }
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

/** The keys of a page of views: a journey's or a route's. */
export function useViewKeys(props: KeysProps): void {
  const addresses = useNodeAddresses();
  const navigate = useNavigate();
  useEffect(() => {
    const heard = (event: KeyboardEvent) => {
      heardKey(event, { ...props, addresses, go: (to) => void navigate(to) });
    };
    document.addEventListener("keydown", heard);
    return () => {
      document.removeEventListener("keydown", heard);
    };
  }, [props, addresses, navigate]);
}

/** A journey's keys: everything its subject offers, less the views its address cannot show now. */
export function useJourneyKeys(props: Omit<KeysProps, "offers" | "unavailable" | "viewAddress">): void {
  const { search } = useLocation();
  const target = useJourneyTarget();
  const { screen, selected, sheetOpen, setSheetOpen, state, steps, onState } = props;
  useViewKeys(
    useMemo(
      () => ({ screen, selected, sheetOpen, setSheetOpen, state, steps, onState, offers: JOURNEY_OFFERS, unavailable: unavailableScreens(target), viewAddress: (to: Screen) => screenSwitched(target, search, to) }),
      [screen, selected, sheetOpen, setSheetOpen, state, steps, onState, target, search],
    ),
  );
}
