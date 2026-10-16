// The controls every view of a journey shares, one row in a fixed order: Detail (how much is
// drawn), Frontier and Decisions (highlights), Filter, and Search, then the view's own end slot.
// On the graph the row floats at the canvas's top left (on a phone, folded into one View menu); on
// the list, the timeline and the walk it sits at the top of the pane. Every control reads and
// writes the journey's one view state (journeys/view.ts), so it means the same on every view:
// Detail, Filter and search choose what is drawn, through the engine's level; Frontier and
// Decisions only light what is there. What a view cannot honour stays, disabled, with its reason
// in its title or, inside the Filter, under it. State shows as a fixed-size badge on the control,
// with the words in its tooltip, so no control changes width; a Filter off its stock settings is
// pressed.
import { useState, type ReactNode } from "react";
import { useLocation } from "react-router";

import { Check, FlagChecks } from "../acting/Controls.tsx";
import { overBytes, TITLE_BYTES_MAX } from "../authoring/limits.ts";
import { STEP_WORDS, stepOf, stepsFor } from "../canvas/ladder.ts";
import type { Step } from "../canvas/model.ts";
import { nodeOf, type Ready } from "../detail/model.ts";
import { entityName } from "../detail/sections.tsx";
import type { Screen } from "../journeys/address.ts";
import { FILTER_FLAGS, filterChanged, filterChips, isStock, stockView, useViewState, withViewState, type ViewFilter } from "../journeys/view.ts";
import { useMedia, PHONE_WIDTH } from "../shell/frame.tsx";
import { Button } from "../ui/kit.tsx";
import { ActiveFilters } from "./ActiveFilters.tsx";
import type { Badge } from "./badge.ts";
import { Menu } from "./Menu.tsx";

/** The jargon term's words, in its tooltip. */
const FRONTIER_WORDS = "What can move now: work that is ready and decisions that can be made, ranked by what they unlock and how soon they are due.";

/** Why the walk through's Frontier is disabled. */
const WALK_FRONTIER = "Every card here is on the frontier. Frontier applies on the other views.";

/** Why a view's class switches are disabled, under them in the Filter. */
const CLASS_REASON: Partial<Record<Screen, string>> = {
  walk: "Every card here is work that applies.",
  timeline: "The timeline has no dates for work that does not apply.",
};

/** What the badges on Filter and Detail say on this view (journeys/requests.ts, `badgesOf`). */
export interface Badges {
  filter?: Badge | undefined;
  detail?: Badge | undefined;
}

function Dotted({ badge, testId, children }: { badge: Badge | undefined; testId: string; children: ReactNode }) {
  return (
    <>
      {children}
      {badge === undefined || badge.count === 0 ? null : (
        <span className="view-badge" aria-hidden="true" data-testid={testId}>
          {badge.count > 9 ? "9+" : badge.count}
        </span>
      )}
    </>
  );
}

/** The tooltip of a control: its words, then what its badge says. */
function tip(words: string, badge: Badge | undefined): string {
  return badge === undefined || badge.count === 0 ? words : `${words}. ${badge.words}`;
}

/** The steps on offer for the journey and the one chosen. */
function useSteps(ready: Ready): { steps: Step[]; step: Step } {
  const { state } = useViewState();
  const nodes = ready.journey.graph.nodes ?? [];
  return { steps: stepsFor(nodes), step: stepOf({ step: state.step }, nodes) };
}

/** C2: the detail levels as radios, coarse to fine; choosing one closes the menu it is in. */
function DetailOptions({ ready, close }: { ready: Ready; close: () => void }) {
  const { state, onState } = useViewState();
  const { steps, step } = useSteps(ready);
  return (
    <div className="opts" role="radiogroup" aria-label="Detail" data-testid="ladder">
      {[...steps].reverse().map((each) => (
        <label key={each} className="opt">
          <input
            type="radio"
            name="detail"
            checked={each === step}
            readOnly
            onClick={() => {
              onState({ ...state, step: each }, true);
              close();
            }}
          />
          <span>{STEP_WORDS[each]}</span>
        </label>
      ))}
    </div>
  );
}

function DetailMenu({ ready, badge }: { ready: Ready; badge: Badge | undefined }) {
  const { step } = useSteps(ready);
  return (
    <Menu
      label={`Detail: ${STEP_WORDS[step]}`}
      title={tip("How much of the journey is drawn, 1 to 4", badge)}
      testId="detail-button"
      role="dialog"
      className="view-control"
      trigger={
        <Dotted badge={badge} testId="detail-badge">
          Detail: {STEP_WORDS[step]} ▾
        </Dotted>
      }
    >
      {(close) => <DetailOptions ready={ready} close={close} />}
    </Menu>
  );
}

/** C9, C1: the one Filter body: only mine, the owner, the flags, then the relevance classes, those this view cannot draw disabled with the reason under them. */
function FilterBody({ ready, screen }: { ready: Ready; screen: Screen }) {
  const { state, onState } = useViewState();
  const { filter } = state;
  const set = (changed: Partial<ViewFilter>) => {
    onState({ ...state, filter: { ...filter, ...changed } });
  };
  const owners = (ready.inputs.deployment.entities ?? []).map((entity) => ({ key: entity.key, text: entityName(ready, entity.key) }));
  const reason = CLASS_REASON[screen];
  return (
    <div className="stack" role="group" aria-label="Filter" data-testid="filter-body">
      <Check label="Only mine" checked={filter.mine} testId="flag-mine" onChange={(mine) => { set({ mine }); }} />
      <select aria-label="Owner" data-testid="filter-owner" value={filter.owner ?? ""} onChange={(event) => { set({ owner: event.target.value === "" ? undefined : event.target.value }); }}>
        <option value="">Owner: any</option>
        {owners.map((owner) => (
          <option key={owner.key} value={owner.key}>
            {owner.text}
          </option>
        ))}
      </select>
      <FlagChecks options={FILTER_FLAGS} chosen={filter.flags} onChange={(flags) => { set({ flags }); }} />
      <hr />
      <Check label="Show conditional" checked={filter.conditional} testId="show-conditional" disabled={screen === "walk"} onChange={(conditional) => { set({ conditional }); }} />
      <Check label="Show not relevant" checked={filter.notRelevant} testId="show-not-relevant" disabled={reason !== undefined} onChange={(notRelevant) => { set({ notRelevant }); }} />
      {reason === undefined ? null : (
        <span className="muted small" data-testid="filter-disabled-reason">
          {reason}
        </span>
      )}
    </div>
  );
}

function FilterMenu({ ready, screen, badge }: { ready: Ready; screen: Screen; badge: Badge | undefined }) {
  const { state } = useViewState();
  return (
    <Menu
      label="Filter"
      title={tip("Filter what is drawn", badge)}
      testId="filter-button"
      role="dialog"
      className={filterChanged(state.filter) ? "view-control menu-pressed" : "view-control"}
      trigger={
        <Dotted badge={badge} testId="filter-badge">
          Filter ▾
        </Dotted>
      }
    >
      {() => (
        <div className="stack" data-testid="filter-panel">
          <FilterBody ready={ready} screen={screen} />
        </div>
      )}
    </Menu>
  );
}

/** C2, C9: the search, which narrows every view to the engine's text matches. Keyed by the text, so a change from outside (a chip, Reset) shows. */
function Search({ onDone }: { onDone?: () => void }) {
  const { state, onState } = useViewState();
  return (
    <SearchBox
      key={state.text}
      text={state.text}
      onSearch={(text) => {
        onState({ ...state, text });
        onDone?.();
      }}
    />
  );
}

function SearchBox({ text, onSearch }: { text: string; onSearch: (text: string) => void }) {
  const [typed, setTyped] = useState(text);
  return (
    <form
      className="toolbar-search"
      role="search"
      onSubmit={(event) => {
        event.preventDefault();
        // The engine reads the search as a title (PRACTICES, Explicit limits): a longer one never leaves the box.
        const problem = overBytes(typed.trim(), TITLE_BYTES_MAX, "The search");
        const input = event.currentTarget.elements.namedItem("q");
        if (input instanceof HTMLInputElement) {
          input.setCustomValidity(problem ?? "");
          input.reportValidity();
        }
        if (problem === undefined) {
          onSearch(typed.trim());
        }
      }}
    >
      <input
        type="search"
        aria-label="Search"
        placeholder="Search"
        title="Search titles, descriptions, notes and resources, then Enter  /"
        data-testid="toolbar-search"
        data-search-input=""
        name="q"
        value={typed}
        onChange={(event) => {
          event.target.setCustomValidity("");
          setTyped(event.target.value);
        }}
      />
    </form>
  );
}

/** C10, C12: Frontier and Decisions: highlights that never change the level, only what stands out (on the list and the walk through, Decisions also narrows the rows or cards to the decisions). The walk through's every card is on the frontier, so its Frontier is disabled. */
function Highlights({ ready, screen }: { ready: Ready; screen: Screen }) {
  const { state, onState } = useViewState();
  const decisionCount = ready.derived.acting_frontier.filter((key) => nodeOf(ready, key)?.kind === "decision").length;
  const walk = screen === "walk";
  return (
    <>
      <button
        type="button"
        className="chip"
        aria-pressed={state.frontier}
        disabled={walk}
        title={walk ? WALK_FRONTIER : `Frontier  Shift F. ${String(ready.derived.acting_frontier.length)} can move now. ${FRONTIER_WORDS}`}
        data-testid="chip-frontier"
        onClick={() => { onState({ ...state, frontier: !state.frontier }); }}
      >
        Frontier
      </button>
      <button
        type="button"
        className="chip"
        aria-pressed={state.decisions}
        title={`Decisions  d. ${String(decisionCount)} can be made now.`}
        data-testid="chip-decisions"
        onClick={() => { onState({ ...state, decisions: !state.decisions }); }}
      >
        Decisions
      </button>
    </>
  );
}

/** Reset the view: the way back from an empty view, offered only off the stock view. Frontier stays (it hides nothing). */
export function ResetView() {
  const { state, onState } = useViewState();
  return isStock(state) ? null : (
    <Button ghost title="Back to All, the stock filter, no search and Decisions off" data-testid="reset-view" onClick={() => { onState(stockView(state), true); }}>
      Reset the view
    </Button>
  );
}

export interface ViewControlsProps {
  ready: Ready;
  screen: Screen;
  /** What the view's filter and detail hide, as badges. */
  badges?: Badges | undefined;
  /** What a screen puts at the row's end (the list's Columns and count). */
  end?: ReactNode;
}

/** The graph's controls on a phone, where the canvas has room for one button: the same controls in a View menu, with the detail and filter badges on it. */
function PhoneControls({ ready, screen, badges }: { ready: Ready; screen: Screen; badges: Badges | undefined }) {
  const shown = [badges?.detail, badges?.filter].flatMap((badge) => (badge === undefined || badge.count === 0 ? [] : [badge]));
  const total = { count: shown.reduce((sum, badge) => sum + badge.count, 0), words: shown.map((badge) => badge.words).join(". ") };
  return (
    <div className="view-controls row" role="toolbar" aria-label="View controls" data-testid="view-controls">
      <Menu
        label="View"
        title={tip("Detail, highlights, filter and search", total)}
        testId="view-button"
        role="dialog"
        className="view-control"
        trigger={
          <Dotted badge={total} testId="view-badge">
            View ▾
          </Dotted>
        }
      >
        {(close) => (
          <div className="stack" data-testid="view-panel-phone">
            <DetailOptions ready={ready} close={close} />
            <div className="row">
              <Highlights ready={ready} screen={screen} />
            </div>
            <FilterBody ready={ready} screen={screen} />
            <Search onDone={close} />
          </div>
        )}
      </Menu>
    </div>
  );
}

export function ViewControls({ ready, screen, badges, end }: ViewControlsProps) {
  // The graph's controls float over the canvas, where a phone has room for one: they fold into a View menu.
  const phone = useMedia(PHONE_WIDTH);
  if (screen === "graph" && phone) {
    return <PhoneControls ready={ready} screen={screen} badges={badges} />;
  }
  return (
    <div className="view-controls row" role="toolbar" aria-label="View controls" data-testid="view-controls">
      <DetailMenu ready={ready} badge={badges?.detail} />
      <Highlights ready={ready} screen={screen} />
      <FilterMenu ready={ready} screen={screen} badge={badges?.filter} />
      <Search />
      {end === undefined ? null : <div className="view-controls-end">{end}</div>}
    </div>
  );
}

/** The row at the top of a list, timeline or walk pane, with the Filter's settings and the search as removable chips. */
export function ViewBar({ ready, screen, end, badges }: { ready: Ready; screen: Screen; end?: ReactNode; badges?: Badges | undefined }) {
  const { pathname, search } = useLocation();
  const { state } = useViewState();
  const chips = filterChips(state, (key) => entityName(ready, key)).map((chip) => ({ id: chip.id, label: chip.label, without: `${pathname}${withViewState(search, chip.without)}` }));
  return (
    <div className="view-bar stack">
      <ViewControls ready={ready} screen={screen} end={end} badges={badges} />
      <ActiveFilters filters={chips} />
    </div>
  );
}
