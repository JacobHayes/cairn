// The journey toolbar (2.3): the page tabs at the left (NEXT and PLAN, each with its count),
// the chips (DECISIONS, FILTER with its count, and the search), and the projection switcher
// always at the right end, so the right edge never moves from one projection to the next. A
// projection that does not apply to the page is left out, never disabled. The chips mean the
// same on every projection, and are always there: they are the address's `decisions`, `mine`
// and `q`, wired to each projection's own filters (acting/address.ts, canvas/settings.ts). On
// the timeline they narrow its rows; on the graph the search finds a node, and with DECISIONS
// on there is only the decisions to show, so the filter says so.
import { useState } from "react";
import { Link, useLocation, useNavigate } from "react-router";

import { ACTING_KINDS, listFrom, listPath, nextFrom, nextPath, triageFrom, triagePath } from "../acting/address.ts";
import { Check, Checks } from "../acting/Controls.tsx";
import { ListFilters } from "../acting/ListFilters.tsx";
import { NextControls } from "../acting/NextScreen.tsx";
import { TriageControls } from "../acting/TriageScreen.tsx";
import { GraphFilters } from "../canvas/KindToggles.tsx";
import { canvasPath, viewFrom } from "../canvas/settings.ts";
import { nodeOf, type Ready } from "../detail/model.ts";
import {
  PAGE_PROJECTIONS,
  carriedSearch,
  pagePath,
  withMineFlipped,
  withParam,
  type JourneyPage,
  type Projection,
} from "../journeys/address.ts";
import { recalledProjection } from "../journeys/memory.ts";
import { Menu } from "./Menu.tsx";
import { activeFilters } from "./filters.ts";

const PAGE_WORDS: Record<JourneyPage, string> = { next: "Next", plan: "Plan" };
const COUNT_WORDS: Record<JourneyPage, string> = { next: "Nodes to act on now", plan: "Nodes in scope" };
const PROJECTION_WORDS: Record<Projection, string> = { graph: "Graph", list: "List", timeline: "Timeline", cards: "Cards" };

/** The filter's words where DECISIONS leaves nothing to choose between. */
function DecisionsOnly() {
  return <p className="muted small">Only decisions are shown. Turn Decisions off to filter by kind.</p>;
}

/** What the filter holds on this projection (each projection's own controls, mounted as they were). */
function FilterBody({ ready, page, projection, node, search }: { ready: Ready; page: JourneyPage; projection: Projection; node: string | undefined; search: string }) {
  const navigate = useNavigate();
  const journey = ready.journey.header.id;
  const params = new URLSearchParams(search);
  if (page === "next" && projection === "list") {
    return <NextControls settings={nextFrom(params)} onChange={(next) => void navigate(nextPath(journey, next, node))} />;
  }
  if (page === "next") {
    return <TriageControls settings={triageFrom(params)} onChange={(next) => void navigate(triagePath(journey, next, node))} />;
  }
  if (projection === "timeline") {
    const { kinds, decisions, flags } = listFrom(params);
    return (
      <>
        <Check label="Mine" checked={flags.includes("mine")} testId="mine" onChange={() => { void navigate(pagePath(journey, page, projection, node, withMineFlipped(search))); }} />
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
            onChange={(chosen) => { void navigate(pagePath(journey, page, projection, node, withParam(search, "kind", chosen.length === 0 ? undefined : chosen.join(",")))); }}
          />
        )}
      </>
    );
  }
  if (projection === "list") {
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
  /** A page, or the Summary page, which is on neither tab and has no projections. */
  page: JourneyPage | "summary";
  projection: Projection | undefined;
  node: string | undefined;
  /** What PLAN counts: the nodes in scope, once the summary has been read. */
  planCount: number | undefined;
  /** The graph's find: the node a search names, opened on the canvas. */
  onFind: (text: string) => void;
}

function Tabs({ ready, page, projection, planCount }: { ready: Ready; page: JourneyPage | "summary"; projection: Projection | undefined; planCount: number | undefined }) {
  const { search } = useLocation();
  const journey = ready.journey.header.id;
  const counts: Record<JourneyPage, number | undefined> = { next: ready.derived.acting_frontier.length, plan: planCount };
  // The page's own tab stays on the projection shown, which is remembered only after it renders.
  const target = (each: JourneyPage) => (each === page && projection !== undefined ? projection : recalledProjection(journey, each));
  // The tab you are on keeps the address as it is, settings and all.
  const query = (each: JourneyPage) => (each === page ? search : carriedSearch(search, each, target(each)));
  return (
    <nav className="journey-tabs row" aria-label="Journey pages" data-testid="journey-tabs">
      {(["next", "plan"] as const).map((each) => (
        <Link
          key={each}
          className="journey-tab"
          aria-current={each === page ? "page" : undefined}
          data-testid={`tab-${each}`}
          to={pagePath(journey, each, target(each), undefined, query(each))}
        >
          {PAGE_WORDS[each]}
          {counts[each] === undefined ? null : (
            <span className="journey-tab-count" data-testid={`tab-${each}-count`} title={COUNT_WORDS[each]}>
              {counts[each]}
            </span>
          )}
        </Link>
      ))}
    </nav>
  );
}

function Switcher({ journey, page, projection, node }: { journey: string; page: JourneyPage; projection: Projection; node: string | undefined }) {
  const { search } = useLocation();
  return (
    <nav className="journey-switcher" aria-label="Projection" data-testid="projection-switcher">
      {PAGE_PROJECTIONS[page].map((each) => (
        <Link
          key={each}
          className="journey-switch"
          aria-current={each === projection ? "page" : undefined}
          data-testid={`projection-${each}`}
          to={pagePath(journey, page, each, node, each === projection ? search : carriedSearch(search, page, each))}
        >
          {PROJECTION_WORDS[each]}
        </Link>
      ))}
    </nav>
  );
}

export function JourneyToolbar({ ready, page, projection, node, planCount, onFind }: ToolbarProps) {
  const { pathname, search } = useLocation();
  const navigate = useNavigate();
  const journey = ready.journey.header.id;
  const params = new URLSearchParams(search);
  const decisions = params.get("decisions") === "1";
  const decisionCount = ready.derived.acting_frontier.filter((key) => nodeOf(ready, key)?.kind === "decision").length;
  const here = page === "summary" || projection === undefined ? undefined : { page, projection };
  const filters = here === undefined ? [] : activeFilters(journey, here.page, here.projection, search, node);
  const set = (name: string, value: string | undefined) => {
    void navigate(`${pathname}${withParam(search, name, value)}`);
  };
  return (
    <div className="journey-toolbar row" role="toolbar" aria-label="Journey toolbar" data-testid="journey-toolbar" data-page={page} data-projection={projection ?? ""}>
      <Tabs ready={ready} page={page} projection={projection} planCount={planCount} />
      {here === undefined ? null : (
        <div className="journey-chips">
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
                <FilterBody ready={ready} page={here.page} projection={here.projection} node={node} search={search} />
              </div>
            )}
          </Menu>
          <Search
            key={params.get("q") ?? ""}
            text={params.get("q") ?? ""}
            onSearch={(text) => {
              // The graph's search finds a node; the others filter by text.
              if (here.projection === "graph") {
                onFind(text);
              } else {
                set("q", text === "" ? undefined : text);
              }
            }}
          />
        </div>
      )}
      {here === undefined ? null : <Switcher journey={journey} page={here.page} projection={here.projection} node={node} />}
    </div>
  );
}
