// The journey toolbar (2.3): the views at the left as one segmented control (GRAPH, LIST,
// TIMELINE, SUMMARY), the chips (FRONTIER with its count, DECISIONS, FILTER with its count, and
// the search), and the Walk through button at the right end. The chips mean the same on every
// view: they are the address's `frontier`, `decisions`, `mine` and `q`, wired to each view's own
// filters (acting/address.ts, canvas/settings.ts). On the timeline they narrow its rows; on the
// graph the search finds a node, and with DECISIONS on there is only the decisions to show, so
// the filter says so. On the Summary there are no chips.
import { useState } from "react";
import { Link, useLocation, useNavigate } from "react-router";

import { ACTING_KINDS, listFrom, listPath, triageFrom, triagePath } from "../acting/address.ts";
import { Check, Checks } from "../acting/Controls.tsx";
import { ListFilters } from "../acting/ListFilters.tsx";
import { TriageControls } from "../acting/TriageScreen.tsx";
import { GraphFilters } from "../canvas/KindToggles.tsx";
import { canvasPath, viewFrom } from "../canvas/settings.ts";
import { nodeOf, type Ready } from "../detail/model.ts";
import { carriedSearch, chipApplies, pagePath, withMineFlipped, withParam, type Screen } from "../journeys/address.ts";
import { Menu } from "./Menu.tsx";
import { activeFilters } from "./filters.ts";

/** The jargon term's words, on its chip and in the strip. */
export const FRONTIER_WORDS = "What can move now: work that is ready and decisions that can be made, ranked by what they unlock and how soon they are due.";

const VIEWS: { screen: Screen; words: string }[] = [
  { screen: "graph", words: "Graph" },
  { screen: "list", words: "List" },
  { screen: "timeline", words: "Timeline" },
  { screen: "summary", words: "Summary" },
];

/** The filter's words where DECISIONS leaves nothing to choose between. */
function DecisionsOnly() {
  return <p className="muted small">Only decisions are shown. Turn Decisions off to filter by kind.</p>;
}

/** What the filter holds on this screen (each screen's own controls, mounted as they were). */
function FilterBody({ ready, screen, node, search }: { ready: Ready; screen: Screen; node: string | undefined; search: string }) {
  const navigate = useNavigate();
  const journey = ready.journey.header.id;
  const params = new URLSearchParams(search);
  if (screen === "walk") {
    return <TriageControls settings={triageFrom(params)} onChange={(next) => void navigate(triagePath(journey, next, node))} />;
  }
  if (screen === "timeline") {
    const { kinds, decisions, flags } = listFrom(params);
    return (
      <>
        <Check label="Mine" checked={flags.includes("mine")} testId="mine" onChange={() => { void navigate(pagePath(journey, screen, node, withMineFlipped(search))); }} />
        {decisions ? (
          <DecisionsOnly />
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
  return <GraphFilters view={viewFrom(params)} onChange={(next) => void navigate(canvasPath(journey, next, node))} />;
}

function Search({ text, onSearch }: { text: string; onSearch: (text: string) => void }) {
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
        data-testid="toolbar-search"
        data-search-input=""
        value={typed}
        onChange={(event) => { setTyped(event.target.value); }}
      />
    </form>
  );
}

export interface ToolbarProps {
  ready: Ready;
  screen: Screen;
  node: string | undefined;
  /** The graph's find: the node a search names, opened on the canvas. */
  onFind: (text: string) => void;
}

/** The views, one segmented control: the page's own address stays as it is, the others carry the chips they read. */
function Views({ journey, screen, node }: { journey: string; screen: Screen; node: string | undefined }) {
  const { search } = useLocation();
  return (
    <nav className="journey-switcher" aria-label="Views" data-testid="journey-views">
      {VIEWS.map((each) => (
        <Link
          key={each.screen}
          className="journey-switch"
          aria-current={each.screen === screen ? "page" : undefined}
          data-testid={`view-${each.screen}`}
          to={pagePath(journey, each.screen, node, each.screen === screen ? search : carriedSearch(search, each.screen))}
        >
          {each.words}
        </Link>
      ))}
    </nav>
  );
}

/** The chips: Frontier (where it means something), Decisions, the filter with its count, and the search. */
function Chips({ ready, screen, node, onFind }: { ready: Ready; screen: Screen; node: string | undefined; onFind: (text: string) => void }) {
  const { pathname, search } = useLocation();
  const navigate = useNavigate();
  const params = new URLSearchParams(search);
  const decisions = params.get("decisions") === "1";
  const frontier = params.get("frontier") === "1";
  const decisionCount = ready.derived.acting_frontier.filter((key) => nodeOf(ready, key)?.kind === "decision").length;
  const filters = activeFilters(ready.journey.header.id, screen, search, node);
  const set = (name: string, value: string | undefined) => {
    void navigate(`${pathname}${withParam(search, name, value)}`);
  };
  return (
    <div className="journey-chips">
      {chipApplies("frontier", screen) ? (
        <button type="button" className="chip" aria-pressed={frontier} title={FRONTIER_WORDS} data-testid="chip-frontier" onClick={() => { set("frontier", frontier ? undefined : "1"); }}>
          Frontier<span className="chip-count">{ready.derived.acting_frontier.length}</span>
        </button>
      ) : null}
      <button type="button" className="chip" aria-pressed={decisions} data-testid="chip-decisions" onClick={() => { set("decisions", decisions ? undefined : "1"); }}>
        Decisions{decisionCount === 0 ? null : <span className="chip-count">{decisionCount}</span>}
      </button>
      <Menu
        label="Filter"
        testId="filter-button"
        role="dialog"
        className="chip-menu"
        trigger={
          <>
            Filter
            {filters.length === 0 ? null : <span className="chip-count" data-testid="filter-count">{filters.length}</span>} ▾
          </>
        }
      >
        {() => (
          <div className="stack" data-testid="filter-panel">
            <FilterBody ready={ready} screen={screen} node={node} search={search} />
          </div>
        )}
      </Menu>
      <Search
        key={params.get("q") ?? ""}
        text={params.get("q") ?? ""}
        onSearch={(text) => {
          // The graph's search finds a node; the others filter by text.
          if (screen === "graph") {
            onFind(text);
          } else {
            set("q", text === "" ? undefined : text);
          }
        }}
      />
    </div>
  );
}

export function JourneyToolbar({ ready, screen, node, onFind }: ToolbarProps) {
  const { search } = useLocation();
  const journey = ready.journey.header.id;
  return (
    <div className="journey-toolbar row" role="toolbar" aria-label="Journey toolbar" data-testid="journey-toolbar" data-screen={screen}>
      <Views journey={journey} screen={screen} node={node} />
      {screen === "summary" ? null : <Chips ready={ready} screen={screen} node={node} onFind={onFind} />}
      <span className="spacer" />
      <Link
        className="button"
        aria-current={screen === "walk" ? "page" : undefined}
        title="Go through the frontier one card at a time"
        data-testid="walk-through"
        to={pagePath(journey, "walk", undefined, carriedSearch(search, "walk"))}
      >
        Walk through
      </Link>
    </div>
  );
}
