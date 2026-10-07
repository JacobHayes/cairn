// A journey's screens, one link each: the canvas (5.2), the acting surfaces (C9 to C11), and
// the read-mostly views (C12, C13, C18). A screen only links to screens that exist
// (briefs/README.md); a later screen adds its own.
//
// The canvas and the read-mostly views each sit at the journey's address plus a segment and
// open node detail at that address plus `/nodes/<key>`. Moving among them keeps an open node
// open, and keeps the address's query: it is the canvas's settings (canvas/settings.ts), which
// the read-mostly views do not read, so coming back to the canvas finds it as it was left. The
// acting surfaces keep their own settings in their query, so they are always linked fresh.
import { Link, useLocation } from "react-router";

import { listPath, nextPath, triagePath, walkthroughPath } from "../acting/address.ts";
import { nodePath } from "../detail/parts.tsx";

export type JourneyScreen = "canvas" | "next" | "list" | "triage" | "walkthrough" | "decisions" | "timeline" | "summary";

/** A screen at the journey's address plus `segment`, carrying the canvas's query and an open node. */
interface Segmented {
  segment: string;
}

/** A screen at an address of its own making, with its own settings. */
interface Addressed {
  path: (journey: string) => string;
}

const SCREENS: ({ screen: JourneyScreen; label: string } & (Segmented | Addressed))[] = [
  { screen: "canvas", label: "Canvas", segment: "" },
  { screen: "next", label: "Next", path: (journey) => nextPath(journey) },
  { screen: "list", label: "List", path: (journey) => listPath(journey) },
  { screen: "triage", label: "Triage", path: (journey) => triagePath(journey) },
  { screen: "walkthrough", label: "Decision walkthrough", path: walkthroughPath },
  { screen: "decisions", label: "Decisions", segment: "decisions" },
  { screen: "timeline", label: "Timeline", segment: "timeline" },
  { screen: "summary", label: "Summary", segment: "summary" },
];

/** Whether `screen` sits at the journey's address plus a segment. */
function segmented(screen: JourneyScreen): boolean {
  return SCREENS.some((each) => each.screen === screen && "segment" in each);
}

/**
 * The tabs between journey `journey`'s screens; the current one is marked. From the canvas or
 * a read-mostly view, `node`'s detail stays open and the canvas's settings ride along.
 */
export function JourneyNav({ journey, current, node }: { journey: string; current: JourneyScreen; node?: string | undefined }) {
  const { search } = useLocation();
  const carry = segmented(current);
  return (
    <nav className="journey-nav row" aria-label="Journey screens" data-testid="journey-nav">
      {SCREENS.map((each) => {
        if (each.screen === current) {
          return (
            <strong key={each.screen} aria-current="page" data-testid={`nav-${each.screen}`}>
              {each.label}
            </strong>
          );
        }
        let to: string | { pathname: string; search: string };
        if ("path" in each) {
          to = each.path(journey);
        } else {
          const screen = each.segment === "" ? `/journeys/${journey}` : `/journeys/${journey}/${each.segment}`;
          to = carry ? { pathname: node === undefined ? screen : nodePath(screen, node), search } : screen;
        }
        return (
          <Link key={each.screen} to={to} data-testid={`nav-${each.screen}`}>
            {each.label}
          </Link>
        );
      })}
    </nav>
  );
}
