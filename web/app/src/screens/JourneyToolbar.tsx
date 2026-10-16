// The journey's view tabs (2.3): the views as one segmented control (GRAPH, LIST, TIMELINE,
// SUMMARY), and the Walk through button at the right end. The controls that act on a view
// (detail, highlights, filter, search) are the view's own (ViewControls.tsx).
import { Link, useLocation } from "react-router";

import { pagePath, type Screen } from "../journeys/address.ts";
import { switchedSearch } from "../journeys/view.ts";

const VIEWS: { screen: Screen; words: string }[] = [
  { screen: "graph", words: "Graph" },
  { screen: "list", words: "List" },
  { screen: "timeline", words: "Timeline" },
  { screen: "summary", words: "Summary" },
];

/** The views, one segmented control: the page's own address stays as it is, the others carry the whole view state. */
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
          to={pagePath(journey, each.screen, node, each.screen === screen ? search : switchedSearch(search, screen, each.screen))}
        >
          {each.words}
        </Link>
      ))}
    </nav>
  );
}

export function JourneyToolbar({ journey, screen, node }: { journey: string; screen: Screen; node: string | undefined }) {
  const { search } = useLocation();
  return (
    <div className="journey-toolbar row" role="toolbar" aria-label="Journey toolbar" data-testid="journey-toolbar" data-screen={screen}>
      <Views journey={journey} screen={screen} node={node} />
      <span className="spacer" />
      <Link
        className="button"
        aria-current={screen === "walk" ? "page" : undefined}
        title="Go through the frontier one card at a time  w"
        data-testid="walk-through"
        to={pagePath(journey, "walk", undefined, switchedSearch(search, screen, "walk"))}
      >
        Walk through
      </Link>
    </div>
  );
}
