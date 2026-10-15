// The controls every view of a journey shares, one row: Detail (how much is drawn), Frontier and
// Decisions (highlights), Filter, and Search. On the graph the row floats at the canvas's top
// left; on the list, the timeline and the walk it sits at the top of the pane. The chips are the
// address's `frontier`, `decisions`, `mine` and `q`, wired to each view's own filters
// (acting/address.ts, canvas/settings.ts). Detail and Filter choose which cards are drawn;
// Frontier and Decisions only highlight cards already there. State shows as a fixed-size badge on
// the control, with the words in its tooltip, so no control changes width.
import { useState, type ReactNode } from "react";
import { useLocation, useNavigate } from "react-router";

import { ACTING_KINDS, listFrom, listPath, triageFrom, triagePath } from "../acting/address.ts";
import { Check, Checks } from "../acting/Controls.tsx";
import { ListFilters } from "../acting/ListFilters.tsx";
import { TriageControls } from "../acting/TriageScreen.tsx";
import { STEP_WORDS } from "../canvas/ladder.ts";
import type { Step } from "../canvas/model.ts";
import { canvasPath, viewFrom } from "../canvas/settings.ts";
import { nodeOf, type Ready } from "../detail/model.ts";
import { useMedia, PHONE_WIDTH } from "../shell/frame.tsx";
import { chipApplies, pagePath, withMineFlipped, withParam, type Screen } from "../journeys/address.ts";
import { DETAILS } from "../timeline/rows.ts";
import { timelineFrom } from "../timeline/TimelineView.tsx";
import { ActiveFilters } from "./ActiveFilters.tsx";
import { activeFilters } from "./filters.ts";
import { Menu } from "./Menu.tsx";

/** The jargon term's words, in its tooltip. */
const FRONTIER_WORDS = "What can move now: work that is ready and decisions that can be made, ranked by what they unlock and how soon they are due.";

/** A state shown as a small count on a control, with the words that go in its tooltip. */
export interface Badge {
  count: number;
  words: string;
}

/** The detail levels on offer (coarse to fine), the one chosen, and what choosing does. */
export interface DetailChoice {
  steps: readonly Step[];
  step: Step;
  onStep: (step: Step) => void;
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

/** The detail levels as radios, coarse to fine; choosing one closes the menu it is in. */
function DetailOptions({ choice, close }: { choice: DetailChoice; close: () => void }) {
  return (
    <div className="opts" role="radiogroup" aria-label="Detail" data-testid="ladder">
      {[...choice.steps].reverse().map((each) => (
        <label key={each} className="opt">
          <input
            type="radio"
            name="detail"
            checked={each === choice.step}
            onChange={() => {
              choice.onStep(each);
              close();
            }}
          />
          <span>{STEP_WORDS[each]}</span>
        </label>
      ))}
    </div>
  );
}

function DetailMenu({ choice, badge }: { choice: DetailChoice; badge: Badge | undefined }) {
  return (
    <Menu
      label={`Detail: ${STEP_WORDS[choice.step]}`}
      title={tip("How much of the journey is drawn, 1 to 4", badge)}
      testId="detail-button"
      role="dialog"
      className="view-control"
      trigger={
        <Dotted badge={badge} testId="detail-badge">
          Detail: {STEP_WORDS[choice.step]} ▾
        </Dotted>
      }
    >
      {(close) => <DetailOptions choice={choice} close={close} />}
    </Menu>
  );
}

/** What the filter holds on this screen (each screen's own controls, mounted as they were). */
function FilterBody({ ready, screen, node, search }: { ready: Ready; screen: Screen; node: string | undefined; search: string }) {
  const navigate = useNavigate();
  const journey = ready.journey.header.id;
  const params = new URLSearchParams(search);
  if (screen === "walk") {
    // The walk through does not read Frontier or the Filter's switch, but the views it returns to do: keep them.
    const dormant = (path: string) => {
      const [base = path, query = ""] = path.split("?");
      const kept = new URLSearchParams(query);
      for (const name of ["frontier", "show"]) {
        const value = params.get(name);
        if (value !== null) {
          kept.set(name, value);
        }
      }
      return kept.size === 0 ? base : `${base}?${kept.toString()}`;
    };
    return <TriageControls settings={triageFrom(params)} onChange={(next) => void navigate(dormant(triagePath(journey, next, node)))} />;
  }
  if (screen === "timeline") {
    const { kinds, decisions, flags } = listFrom(params);
    return (
      <>
        <Check label="Mine" checked={flags.includes("mine")} testId="mine" onChange={() => { void navigate(pagePath(journey, screen, node, withMineFlipped(search))); }} />
        {decisions ? (
          <p className="muted small">Only decisions are shown. Turn Decisions off to filter by kind.</p>
        ) : (
          <Checks
            legend="Kinds"
            options={ACTING_KINDS}
            chosen={kinds}
            words={(kind) => kind}
            testId="kind"
            stacked
            onChange={(chosen) => { void navigate(pagePath(journey, screen, node, withParam(search, "kind", chosen.length === 0 ? undefined : chosen.join(",")))); }}
          />
        )}
      </>
    );
  }
  if (screen === "list") {
    return <ListFilters view={ready} settings={listFrom(params)} onChange={(next) => void navigate(listPath(journey, next, node))} />;
  }
  const view = viewFrom(params);
  return (
    <div className="stack" role="group" aria-label="What the graph shows">
      <Check label="Show conditional" checked={view.undecided} testId="show-conditional" onChange={(undecided) => { void navigate(canvasPath(journey, { ...view, undecided }, node)); }} />
      <Check label="Show not relevant" checked={view.notRelevant} testId="show-not-relevant" onChange={(notRelevant) => { void navigate(canvasPath(journey, { ...view, notRelevant }, node)); }} />
    </div>
  );
}

function Search({ text, find, onSearch }: { text: string; find: boolean; onSearch: (text: string) => void }) {
  const [typed, setTyped] = useState(text);
  return (
    <form
      className="toolbar-search"
      role="search"
      onSubmit={(event) => {
        event.preventDefault();
        onSearch(typed.trim());
      }}
    >
      <input
        type="search"
        aria-label="Search"
        placeholder="Search"
        title={find ? "Find a node by title, then Enter  /" : "Filter by text, then Enter  /"}
        data-testid="toolbar-search"
        data-search-input=""
        value={typed}
        onChange={(event) => { setTyped(event.target.value); }}
      />
    </form>
  );
}

/** Frontier and Decisions: highlights that change nothing drawn, only what stands out. */
function Highlights({ ready, screen }: { ready: Ready; screen: Screen }) {
  const { pathname, search } = useLocation();
  const navigate = useNavigate();
  const params = new URLSearchParams(search);
  const decisions = params.get("decisions") === "1";
  const frontier = params.get("frontier") === "1";
  const decisionCount = ready.derived.acting_frontier.filter((key) => nodeOf(ready, key)?.kind === "decision").length;
  const set = (name: string, value: string | undefined) => {
    void navigate(`${pathname}${withParam(search, name, value)}`);
  };
  return (
    <>
      {chipApplies("frontier", screen) ? (
        <button
          type="button"
          className="chip"
          aria-pressed={frontier}
          title={`Frontier  Shift F. ${String(ready.derived.acting_frontier.length)} can move now. ${FRONTIER_WORDS}`}
          data-testid="chip-frontier"
          onClick={() => { set("frontier", frontier ? undefined : "1"); }}
        >
          Frontier
        </button>
      ) : null}
      <button
        type="button"
        className="chip"
        aria-pressed={decisions}
        title={`Decisions  d. ${String(decisionCount)} can be made now.`}
        data-testid="chip-decisions"
        onClick={() => { set("decisions", decisions ? undefined : "1"); }}
      >
        Decisions
      </button>
    </>
  );
}

export interface ViewControlsProps {
  ready: Ready;
  screen: Screen;
  node: string | undefined;
  /** The graph's detail levels; the timeline's are read from the address. */
  detail?: DetailChoice | undefined;
  /** What the graph's filter and detail hide, as badges. */
  badges?: { filter?: Badge | undefined; detail?: Badge | undefined } | undefined;
  /** The graph's find: the node a search names, opened on the canvas. */
  onFind?: ((text: string) => void) | undefined;
  /** What a screen puts at the row's end (the list's Columns and count). */
  end?: ReactNode;
}

/** The timeline's detail levels, read from and written to the address. */
function useTimelineDetail(screen: Screen, search: string): DetailChoice | undefined {
  const navigate = useNavigate();
  const { pathname } = useLocation();
  if (screen !== "timeline") {
    return undefined;
  }
  return {
    steps: DETAILS,
    step: timelineFrom(new URLSearchParams(search)).detail,
    onStep: (step) => void navigate(`${pathname}${withParam(search, "detail", step === "stages" ? undefined : step)}`),
  };
}

/** The search box: the graph's finds a node (and the menu it is in closes), the others filter by text. Keyed by screen: text typed for the graph's find is not a filter the next view holds. */
function Find({ screen, onFind, onDone }: { screen: Screen; onFind: ((text: string) => void) | undefined; onDone?: () => void }) {
  const { pathname, search } = useLocation();
  const navigate = useNavigate();
  const text = new URLSearchParams(search).get("q") ?? "";
  return (
    <Search
      key={`${screen}:${text}`}
      text={text}
      find={screen === "graph"}
      onSearch={(typed) => {
        if (screen === "graph") {
          onFind?.(typed);
          onDone?.();
        } else {
          void navigate(`${pathname}${withParam(search, "q", typed === "" ? undefined : typed)}`);
        }
      }}
    />
  );
}

/** The graph's controls on a phone, where the canvas has room for one button: the same controls in a View menu, with the detail and filter badges on it. */
function PhoneControls({ ready, screen, node, chosen, badges, filter, onFind }: {
  ready: Ready;
  screen: Screen;
  node: string | undefined;
  chosen: DetailChoice | undefined;
  badges: ViewControlsProps["badges"];
  filter: Badge | undefined;
  onFind: ((text: string) => void) | undefined;
}) {
  const { search } = useLocation();
  const shown = [badges?.detail, filter].flatMap((badge) => (badge === undefined || badge.count === 0 ? [] : [badge]));
  const total = { count: shown.reduce((sum, badge) => sum + badge.count, 0), words: shown.map((badge) => badge.words).join(". ") };
  return (
    <div className="view-controls row" role="toolbar" aria-label="View controls" data-testid="view-controls">
      <Menu
        label="View"
        title={tip("Detail, highlights, filter and find", total)}
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
            {chosen === undefined ? null : <DetailOptions choice={chosen} close={close} />}
            <div className="row">
              <Highlights ready={ready} screen={screen} />
            </div>
            <FilterBody ready={ready} screen={screen} node={node} search={search} />
            <Find screen={screen} onFind={onFind} onDone={close} />
          </div>
        )}
      </Menu>
    </div>
  );
}

export function ViewControls({ ready, screen, node, detail, badges, onFind, end }: ViewControlsProps) {
  const { search } = useLocation();
  // The graph's controls float over the canvas, where a phone has room for one: they fold into a View menu.
  const phone = useMedia(PHONE_WIDTH);
  const collapsed = screen === "graph" && phone;
  const timelineDetail = useTimelineDetail(screen, search);
  const chosen = detail ?? timelineDetail;
  const active = activeFilters(ready.journey.header.id, screen, search, node);
  const filter: Badge | undefined = badges?.filter ?? (active.length === 0 ? undefined : { count: active.length, words: `${String(active.length)} on` });
  if (collapsed) {
    return <PhoneControls ready={ready} screen={screen} node={node} chosen={chosen} badges={badges} filter={filter} onFind={onFind} />;
  }
  return (
    <div className="view-controls row" role="toolbar" aria-label="View controls" data-testid="view-controls">
      {chosen === undefined ? null : <DetailMenu choice={chosen} badge={badges?.detail} />}
      <Highlights ready={ready} screen={screen} />
      <Menu
        label="Filter"
        title={tip("Filter what is drawn", filter)}
        testId="filter-button"
        role="dialog"
        className="view-control"
        trigger={
          <Dotted badge={filter} testId="filter-badge">
            Filter ▾
          </Dotted>
        }
      >
        {() => (
          <div className="stack" data-testid="filter-panel">
            <FilterBody ready={ready} screen={screen} node={node} search={search} />
          </div>
        )}
      </Menu>
      <Find screen={screen} onFind={onFind} />
      {end === undefined ? null : <div className="view-controls-end">{end}</div>}
    </div>
  );
}

/** The row at the top of a list, timeline or walk pane, with the filters it has on as removable chips. */
export function ViewBar({ ready, screen, node, end, badges }: { ready: Ready; screen: Screen; node: string | undefined; end?: ReactNode; badges?: ViewControlsProps["badges"] }) {
  const { search } = useLocation();
  return (
    <div className="view-bar stack">
      <ViewControls ready={ready} screen={screen} node={node} end={end} badges={badges} />
      <ActiveFilters filters={activeFilters(ready.journey.header.id, screen, search, node)} />
    </div>
  );
}
