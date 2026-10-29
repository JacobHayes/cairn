// The controls every view of a journey or a route shares, one row in a fixed order: Depth (how
// deep is drawn), Frontier (emphasis), Filter, and Search, then the view's own end slot. A route
// has no state, owner or relevance to decide, so it offers Depth (with Fade), the Filter's
// conditional class and Search; the rest is left out, not disabled (it is not a setting a
// sibling view of the same subject honours). On the graph the
// row floats at the canvas's top left; on the list, the
// timeline and the walk it sits at the top of the pane; on a phone it is one Controls menu in every view. Every control reads and writes the
// journey's one view state (journeys/view.ts), so it means the same on every view: Depth, Filter
// and search choose what is drawn, through the engine's level; Frontier only lights what is
// there, and so does Depth's Fade, which draws every kind and dims those below the depth instead
// of leaving them out. What a view cannot honour stays, disabled, with its reason
// in its title or, inside the Filter, under it. State shows as a fixed-size badge on the control,
// with the words in its tooltip, so no control changes width; a Filter off its stock settings is
// pressed.
import { useEffect, useRef, useState, type ReactNode } from "react";
import { useLocation } from "react-router";

import { Check, Counted, FlagChecks } from "../acting/Controls.tsx";
import { STEP_WORDS, stepOf, stepsFor } from "../canvas/ladder.ts";
import type { Step } from "../canvas/model.ts";
import { useValueProblems } from "../authoring/form-problems.ts";
import type { GraphNode, Ready } from "../detail/model.ts";
import { identityName } from "../detail/sections.tsx";
import type { Screen } from "../journeys/address.ts";
import type { HiddenCounts } from "../journeys/requests.ts";
import { canDraw, fadeOn, FILTER_FLAGS, JOURNEY_OFFERS, RELEVANCE_CLASSES, ROUTE_OFFERS, SCREEN_VIEWS, filterChanged, filterChips, isStock, searchCheck, stockView, useViewState, withViewState, type RelevanceClass, type ViewFilter, type ViewOffers } from "../journeys/view.ts";
import { useMedia, PHONE_WIDTH } from "../shell/frame.tsx";
import { Button } from "../ui/kit.tsx";
import { ActiveFilters } from "./ActiveFilters.tsx";
import type { Badge } from "./badge.ts";
import { Menu } from "./Menu.tsx";
import { Tip } from "../ui/Tip.tsx";

/** What the view hides, as it says it: the badges on Filter and Depth, and in the open Filter the count each setting hides (journeys/requests.ts, `badgesOf`). */
export interface Badges {
  hidden?: HiddenCounts | undefined;
  filter?: Badge | undefined;
  depth?: Badge | undefined;
}

function Dotted({ badge, testId, children }: { badge: Badge | undefined; testId: string; children: ReactNode }) {
  return (
    <>
      {children}
      {badge === undefined || badge.count === 0 ? null : (
        <span className="view-badge" aria-hidden="true" data-overhang="" data-testid={testId}>
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

/** What the controls are for: a graph's nodes, what its subject offers, and for a journey the journey, whose owners and frontier the journey's controls read. */
export interface ViewSubject {
  nodes: readonly GraphNode[];
  offers: ViewOffers;
  journey: Ready | undefined;
}

/** A journey's subject: everything the controls offer. */
export function journeySubject(ready: Ready): ViewSubject {
  return { nodes: ready.journey.graph.nodes ?? [], offers: JOURNEY_OFFERS, journey: ready };
}

/** A route's subject: its nodes, and what a route offers. */
export function routeSubject(nodes: readonly GraphNode[]): ViewSubject {
  return { nodes, offers: ROUTE_OFFERS, journey: undefined };
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
      {!SCREEN_VIEWS[screen].fade ? null : (
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
  const fading = fadeOn(screen, state);
  return (
    <Menu
      label={`Depth: ${STEP_WORDS[step]}`}
      tip={tip(`How deep ${subject.journey === undefined ? "the route" : "the journey"} is drawn: each step draws down to its kind, 1 to 4${fading ? ". The rest is faded, not hidden" : ""}`, badge)}
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
 * C9, C1: the one Filter body: what the subject offers of only mine, the owner and the flags,
 * then the relevance classes it offers, those this view cannot draw disabled with the reason
 * under them (a route offers the conditional class alone).
 */
export function FilterBody({ subject, screen, hidden = { classes: {}, flags: {} } }: { subject: ViewSubject; screen: Screen; hidden?: Badges["hidden"] }) {
  const { state, onState } = useViewState();
  const { filter } = state;
  const set = (changed: Partial<ViewFilter>) => {
    onState({ ...state, filter: { ...filter, ...changed } });
  };
  const { journey, offers } = subject;
  const owners = (journey?.inputs.deployment.identities ?? []).map((identity) => ({ key: identity.key, text: journey === undefined ? identity.key : identityName(journey, identity.key) }));
  const classes = RELEVANCE_CLASSES.filter((each) => each !== "not_relevant" || offers.notRelevant);
  const reasons = [...new Set(classes.flatMap((each) => SCREEN_VIEWS[screen].classes[each] ?? []))];
  const classBox = (each: RelevanceClass, label: string, testId: string, checked: boolean) => (
    <Check
      label={<Counted label={label} count={hidden.classes[each]} testId={testId} />}
      checked={checked}
      testId={testId}
      disabled={!canDraw(screen, each)}
      onChange={(on) => { set(each === "conditional" ? { conditional: on } : { notRelevant: on }); }}
    />
  );
  return (
    <div className="stack" role="group" aria-label="Filter" data-testid="filter-body">
      {offers.mine ? <Check label={<Counted label="Only mine" count={hidden.flags.mine} testId="flag-mine" />} checked={filter.mine} testId="flag-mine" onChange={(mine) => { set({ mine }); }} /> : null}
      {offers.owner ? (
        <span className="row">
          <select aria-label="Owner" data-testid="filter-owner" value={filter.owner ?? ""} onChange={(event) => { set({ owner: event.target.value === "" ? undefined : event.target.value }); }}>
            <option value="">Owner: any</option>
            {owners.map((owner) => (
              <option key={owner.key} value={owner.key}>
                {owner.text}
              </option>
            ))}
          </select>
          <Counted label={null} count={hidden.owner} testId="filter-owner" />
        </span>
      ) : null}
      {offers.flags ? <FlagChecks options={FILTER_FLAGS} chosen={filter.flags} counts={hidden.flags} onChange={(flags) => { set({ flags }); }} /> : null}
      {offers.mine || offers.owner || offers.flags ? <hr /> : null}
      {classBox("conditional", "Show conditional", "show-conditional", filter.conditional)}
      {offers.notRelevant ? classBox("not_relevant", "Show not relevant", "show-not-relevant", filter.notRelevant) : null}
      {reasons.map((reason) => (
        <span key={reason} className="muted small" data-testid="filter-disabled-reason">
          {reason}
        </span>
      ))}
    </div>
  );
}

function FilterMenu({ subject, screen, badges }: { subject: ViewSubject; screen: Screen; badges: Badges | undefined }) {
  const { state } = useViewState();
  return (
    <Menu
      label="Filter"
      tip={badges?.filter === undefined || badges.filter.count === 0 ? undefined : badges.filter.words}
      testId="filter-button"
      role="dialog"
      className={filterChanged(state.filter) ? "view-control menu-pressed" : "view-control"}
      trigger={
        <Dotted badge={badges?.filter} testId="filter-badge">
          Filter ▾
        </Dotted>
      }
    >
      {() => (
        <div className="stack" data-testid="filter-panel">
          <FilterBody subject={subject} screen={screen} hidden={badges?.hidden} />
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
  // The text submitted and waiting for the engine's reading of it; typing again drops it.
  const [asked, setAsked] = useState<string | undefined>(undefined);
  const input = useRef<HTMLInputElement>(null);
  const check = useValueProblems([searchCheck(asked ?? "")]);
  const problem = check.problems["q"];
  // The engine reads the search as a title: one it refuses never leaves the box, and is said where it is typed.
  useEffect(() => {
    if (asked === undefined || !check.checked) {
      return;
    }
    setAsked(undefined);
    if (problem === undefined) {
      onSearch(asked.trim());
    } else {
      input.current?.setCustomValidity(problem);
      input.current?.reportValidity();
    }
  }, [asked, check.checked, problem, onSearch]);
  return (
    <form
      className="toolbar-search"
      role="search"
      onSubmit={(event) => {
        event.preventDefault();
        setAsked(typed);
      }}
    >
      <Tip tip="Search titles, descriptions, notes and resources, then Enter" keys="/">
        <input
          ref={input}
          type="search"
          aria-label="Search"
          placeholder="Search"
          data-testid="toolbar-search"
          data-search-input=""
          name="q"
          value={typed}
          onChange={(event) => {
            event.target.setCustomValidity("");
            setAsked(undefined);
            setTyped(event.target.value);
            // Emptying the box (its own clear button too) takes an applied search off at once: no filter stays on unseen.
            if (event.target.value === "" && text !== "") {
              onSearch("");
            }
          }}
        />
      </Tip>
    </form>
  );
}

/** C10: Frontier, where the subject offers it: an emphasis that never changes what is drawn, only what stands out. A view it lights nothing on (`SCREEN_VIEWS`) shows it disabled with why. */
function FrontierChip({ subject, screen }: { subject: ViewSubject; screen: Screen }) {
  const { state, onState } = useViewState();
  const { journey } = subject;
  if (!subject.offers.frontier || journey === undefined) {
    return null;
  }
  const off = SCREEN_VIEWS[screen].frontier;
  return (
    <Button
      className="chip"
      aria-pressed={state.frontier}
      held={off}
      tip={`Light up what can move now: ${String(journey.derived.acting_frontier.length)}`}
      keys="Shift F"
      data-testid="chip-frontier"
      data-tour="frontier"
      onClick={() => { onState({ ...state, frontier: !state.frontier }); }}
    >
      Frontier
    </Button>
  );
}

/** What every view says when the depth, filter and search leave nothing to draw, with the way back: the one wording. */
export function NothingMatches() {
  return (
    <>
      Nothing matches the depth, filter and search. <ResetView />
    </>
  );
}

/** The empty view's own paper panel (DESIGN, Panel): the sentence and its way back where the rows or canvas would have been, bordered and ticked like them. */
export function EmptyPanel({ testId, children }: { testId: string; children: ReactNode }) {
  return (
    <div className="node-panel empty-row" role="status" data-testid={testId}>
      {children}
    </div>
  );
}

/** Reset the view: the way back from an empty view, offered only off the stock view. Frontier and Fade stay (they hide nothing). */
export function ResetView() {
  const { state, onState } = useViewState();
  return isStock(state) ? null : (
    <Button ghost tip="Back to Actions depth, the stock filter and no search" data-testid="reset-view" onClick={() => { onState(stockView(state), true); }}>
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

/** The controls on a phone, where a view has room for one button: the same controls in a Controls menu, with the depth and filter badges summed on it, and what the screen puts at the row's end beside it. */
function PhoneControls({ subject, screen, badges, end }: { subject: ViewSubject; screen: Screen; badges: Badges | undefined; end: ReactNode }) {
  const shown = [badges?.depth, badges?.filter].flatMap((badge) => (badge === undefined || badge.count === 0 ? [] : [badge]));
  const total = shown.length === 0 ? undefined : { count: shown.reduce((sum, badge) => sum + badge.count, 0), words: shown.map((badge) => badge.words).join(". ") };
  return (
    <div className="view-controls row" role="toolbar" aria-label="View controls" data-testid="view-controls">
      <Menu
        label="Controls"
        tip={tip(subject.offers.frontier ? "Depth, Frontier, filter and search" : "Depth, filter and search", total)}
        testId="controls-button"
        tour="controls"
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
            <span className="label">Depth</span>
            <DepthOptions subject={subject} screen={screen} close={close} />
            <hr className="menu-rule" />
            {subject.offers.frontier ? (
              <div className="row">
                <FrontierChip subject={subject} screen={screen} />
              </div>
            ) : null}
            <FilterBody subject={subject} screen={screen} hidden={badges?.hidden} />
            <Search onDone={close} />
          </div>
        )}
      </Menu>
      {end === undefined ? null : <div className="view-controls-end">{end}</div>}
    </div>
  );
}

export function ViewControls({ subject, screen, badges, end }: ViewControlsProps) {
  // A phone has room for one button on every view: the controls fold into a Controls menu.
  const phone = useMedia(PHONE_WIDTH);
  if (phone) {
    return <PhoneControls subject={subject} screen={screen} badges={badges} end={end} />;
  }
  return (
    <div className="view-controls row" role="toolbar" aria-label="View controls" data-testid="view-controls">
      <DepthMenu subject={subject} screen={screen} badge={badges?.depth} />
      <FrontierChip subject={subject} screen={screen} />
      <FilterMenu subject={subject} screen={screen} badges={badges} />
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
  const chips = filterChips(state, (key) => (journey === undefined ? key : identityName(journey, key))).map((chip) => ({ id: chip.id, label: chip.label, without: `${pathname}${withViewState(search, chip.without)}` }));
  return (
    <div className="view-bar stack">
      <ViewControls subject={subject} screen={screen} end={end} badges={badges} />
      <ActiveFilters filters={chips} />
    </div>
  );
}
