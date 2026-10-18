// The controls every view of a journey or a route shares, one row in a fixed order: Depth (how
// deep is drawn), Frontier (emphasis), Filter, and Search, then the view's own end slot. A route
// has no state, owner or relevance to decide, so it offers Depth (with Fade), the Filter's
// conditional class and Search; the rest is left out, not disabled (it is not a setting a
// sibling view of the same subject honours). On the graph the
// row floats at the canvas's top left (on a phone, folded into one Controls menu); on the list, the
// timeline and the walk it sits at the top of the pane. Every control reads and writes the
// journey's one view state (journeys/view.ts), so it means the same on every view: Depth, Filter
// and search choose what is drawn, through the engine's level; Frontier only lights what is
// there, and so does Depth's Fade, which draws every kind and dims those below the depth instead
// of leaving them out. What a view cannot honour stays, disabled, with its reason
// in its title or, inside the Filter, under it. State shows as a fixed-size badge on the control,
// with the words in its tooltip, so no control changes width; a Filter off its stock settings is
// pressed.
import { useState, type ReactNode } from "react";
import { useLocation } from "react-router";

import { Check, FlagChecks } from "../acting/Controls.tsx";
import { overBytes, TITLE_BYTES_MAX } from "../authoring/limits.ts";
import { STEP_WORDS, stepOf, stepsFor } from "../canvas/ladder.ts";
import type { Step } from "../canvas/model.ts";
import type { GraphNode, Ready } from "../detail/model.ts";
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

/** What the badges on Filter and Depth say on this view (journeys/requests.ts, `badgesOf`). */
export interface Badges {
  filter?: Badge | undefined;
  depth?: Badge | undefined;
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

/** What the controls are for: a graph's nodes and, for a journey, the journey, whose owners, frontier and state the journey-only controls read. */
export interface ViewSubject {
  nodes: readonly GraphNode[];
  journey: Ready | undefined;
}

/** A journey's subject: everything the controls offer. */
export function journeySubject(ready: Ready): ViewSubject {
  return { nodes: ready.journey.graph.nodes ?? [], journey: ready };
}

/** A route's subject: its nodes alone. */
export function routeSubject(nodes: readonly GraphNode[]): ViewSubject {
  return { nodes, journey: undefined };
}

/** The steps on offer for the subject and the one chosen. */
function useSteps(subject: ViewSubject): { steps: Step[]; step: Step } {
  const { state } = useViewState();
  return { steps: stepsFor(subject.nodes), step: stepOf({ step: state.depth }, subject.nodes) };
}

/**
 * C2: Depth's steps as radios, coarse to fine (keys 1 to 4), then, on a view that can fade, a
 * hairline and the Fade checkbox. Choosing a step closes the menu it is in; the checkbox leaves it
 * open, so the canvas behind can be watched.
 */
function DepthOptions({ subject, screen, close }: { subject: ViewSubject; screen: Screen; close: () => void }) {
  const { state, onState } = useViewState();
  const { steps, step } = useSteps(subject);
  return (
    <div className="stack" data-testid="depth-panel">
      <div className="opts" role="radiogroup" aria-label="Depth" data-testid="ladder">
        {steps.map((each) => (
          <label key={each} className="opt">
            <input
              type="radio"
              name="depth"
              checked={each === step}
              readOnly
              onClick={() => {
                onState({ ...state, depth: each }, !state.fade);
                close();
              }}
            />
            <span>{STEP_WORDS[each]}</span>
          </label>
        ))}
      </div>
      {screen === "walk" ? null : (
        <>
          <hr className="menu-rule" />
          <Check label="Fade the rest instead of hiding it" checked={state.fade} testId="depth-fade" onChange={(fade) => { onState({ ...state, fade }); }} />
        </>
      )}
    </div>
  );
}

function DepthMenu({ subject, screen, badge }: { subject: ViewSubject; screen: Screen; badge: Badge | undefined }) {
  const { state } = useViewState();
  const { step } = useSteps(subject);
  const fading = state.fade && screen !== "walk";
  return (
    <Menu
      label={`Depth: ${STEP_WORDS[step]}`}
      title={tip(`How deep ${subject.journey === undefined ? "the route" : "the journey"} is drawn: each step draws down to its kind, 1 to 4${fading ? ". The rest is faded, not hidden" : ""}`, badge)}
      testId="depth-button"
      role="dialog"
      className="view-control"
      trigger={
        <Dotted badge={badge} testId="depth-badge">
          Depth: {STEP_WORDS[step]} ▾
        </Dotted>
      }
    >
      {(close) => <DepthOptions subject={subject} screen={screen} close={close} />}
    </Menu>
  );
}

/**
 * C9, C1: the one Filter body: for a journey only mine, the owner, the flags, then the relevance
 * classes, those this view cannot draw disabled with the reason under them; for a route, the
 * conditional class alone.
 */
export function FilterBody({ subject, screen }: { subject: ViewSubject; screen: Screen }) {
  const { state, onState } = useViewState();
  const { filter } = state;
  const set = (changed: Partial<ViewFilter>) => {
    onState({ ...state, filter: { ...filter, ...changed } });
  };
  const { journey } = subject;
  const owners = (journey?.inputs.deployment.entities ?? []).map((entity) => ({ key: entity.key, text: journey === undefined ? entity.key : entityName(journey, entity.key) }));
  const reason = CLASS_REASON[screen];
  return (
    <div className="stack" role="group" aria-label="Filter" data-testid="filter-body">
      {journey === undefined ? null : (
        <>
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
        </>
      )}
      <Check label="Show conditional" checked={filter.conditional} testId="show-conditional" disabled={screen === "walk"} onChange={(conditional) => { set({ conditional }); }} />
      {journey === undefined ? null : <Check label="Show not relevant" checked={filter.notRelevant} testId="show-not-relevant" disabled={reason !== undefined} onChange={(notRelevant) => { set({ notRelevant }); }} />}
      {reason === undefined || journey === undefined ? null : (
        <span className="muted small" data-testid="filter-disabled-reason">
          {reason}
        </span>
      )}
    </div>
  );
}

function FilterMenu({ subject, screen, badge }: { subject: ViewSubject; screen: Screen; badge: Badge | undefined }) {
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
          <FilterBody subject={subject} screen={screen} />
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
          // Emptying the box (its own clear button too) takes an applied search off at once: no filter stays on unseen.
          if (event.target.value === "" && text !== "") {
            onSearch("");
          }
        }}
      />
    </form>
  );
}

/** C10: Frontier: an emphasis that never changes what is drawn, only what stands out. The walk through's every card is on the frontier, so its Frontier is disabled. */
function FrontierChip({ ready, screen }: { ready: Ready; screen: Screen }) {
  const { state, onState } = useViewState();
  const walk = screen === "walk";
  return (
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
  );
}

/** Reset the view: the way back from an empty view, offered only off the stock view. Frontier and Fade stay (they hide nothing). */
export function ResetView() {
  const { state, onState } = useViewState();
  return isStock(state) ? null : (
    <Button ghost title="Back to Actions depth, the stock filter and no search" data-testid="reset-view" onClick={() => { onState(stockView(state), true); }}>
      Reset the view
    </Button>
  );
}

export interface ViewControlsProps {
  subject: ViewSubject;
  screen: Screen;
  /** What the view's filter and depth hide, as badges. */
  badges?: Badges | undefined;
  /** What a screen puts at the row's end (the list's Columns and count). */
  end?: ReactNode;
}

/** The graph's controls on a phone, where the canvas has room for one button: the same controls in a Controls menu, with the depth and filter badges on it. */
function PhoneControls({ subject, screen, badges }: { subject: ViewSubject; screen: Screen; badges: Badges | undefined }) {
  const shown = [badges?.depth, badges?.filter].flatMap((badge) => (badge === undefined || badge.count === 0 ? [] : [badge]));
  const total = { count: shown.reduce((sum, badge) => sum + badge.count, 0), words: shown.map((badge) => badge.words).join(". ") };
  return (
    <div className="view-controls row" role="toolbar" aria-label="View controls" data-testid="view-controls">
      <Menu
        label="Controls"
        title={tip(subject.journey === undefined ? "Depth, filter and search" : "Depth, Frontier, filter and search", total)}
        testId="controls-button"
        role="dialog"
        className="view-control"
        trigger={
          <Dotted badge={total} testId="controls-badge">
            Controls ▾
          </Dotted>
        }
      >
        {(close) => (
          <div className="stack" data-testid="controls-panel-phone">
            <span className="muted small">Depth</span>
            <DepthOptions subject={subject} screen={screen} close={close} />
            <hr className="menu-rule" />
            {subject.journey === undefined ? null : (
              <div className="row">
                <FrontierChip ready={subject.journey} screen={screen} />
              </div>
            )}
            <FilterBody subject={subject} screen={screen} />
            <Search onDone={close} />
          </div>
        )}
      </Menu>
    </div>
  );
}

export function ViewControls({ subject, screen, badges, end }: ViewControlsProps) {
  // The graph's controls float over the canvas, where a phone has room for one: they fold into a Controls menu.
  const phone = useMedia(PHONE_WIDTH);
  if (screen === "graph" && phone) {
    return <PhoneControls subject={subject} screen={screen} badges={badges} />;
  }
  return (
    <div className="view-controls row" role="toolbar" aria-label="View controls" data-testid="view-controls">
      <DepthMenu subject={subject} screen={screen} badge={badges?.depth} />
      {subject.journey === undefined ? null : <FrontierChip ready={subject.journey} screen={screen} />}
      <FilterMenu subject={subject} screen={screen} badge={badges?.filter} />
      <Search />
      {end === undefined ? null : <div className="view-controls-end">{end}</div>}
    </div>
  );
}

/** The row at the top of a list, timeline or walk pane, with the Filter's settings as removable chips. */
export function ViewBar({ subject, screen, end, badges }: { subject: ViewSubject; screen: Screen; end?: ReactNode; badges?: Badges | undefined }) {
  const { pathname, search } = useLocation();
  const { state } = useViewState();
  const { journey } = subject;
  const chips = filterChips(state, (key) => (journey === undefined ? key : entityName(journey, key))).map((chip) => ({ id: chip.id, label: chip.label, without: `${pathname}${withViewState(search, chip.without)}` }));
  return (
    <div className="view-bar stack">
      <ViewControls subject={subject} screen={screen} end={end} badges={badges} />
      <ActiveFilters filters={chips} />
    </div>
  );
}
