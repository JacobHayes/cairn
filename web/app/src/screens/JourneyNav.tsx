// A journey's screens, one link each: the canvas (5.2) and the acting surfaces (C9 to C11).
// A screen only links to screens that exist (briefs/README.md); a later screen adds its own.
import { Link } from "react-router";

import { listPath, nextPath, triagePath, walkthroughPath } from "../acting/address.ts";

export type JourneyScreen = "canvas" | "next" | "list" | "triage" | "walkthrough";

const SCREENS: { screen: JourneyScreen; label: string; path: (journey: string) => string }[] = [
  { screen: "canvas", label: "Canvas", path: (journey) => `/journeys/${journey}` },
  { screen: "next", label: "Next", path: (journey) => nextPath(journey) },
  { screen: "list", label: "List", path: (journey) => listPath(journey) },
  { screen: "triage", label: "Triage", path: (journey) => triagePath(journey) },
  { screen: "walkthrough", label: "Decision walkthrough", path: walkthroughPath },
];

export function JourneyNav({ journey, current }: { journey: string; current: JourneyScreen }) {
  return (
    <nav className="journey-nav row" aria-label="Journey screens" data-testid="journey-nav">
      {SCREENS.map(({ screen, label, path }) =>
        screen === current ? (
          <strong key={screen} aria-current="page" data-testid={`nav-${screen}`}>
            {label}
          </strong>
        ) : (
          <Link key={screen} to={path(journey)} data-testid={`nav-${screen}`}>
            {label}
          </Link>
        ),
      )}
    </nav>
  );
}
