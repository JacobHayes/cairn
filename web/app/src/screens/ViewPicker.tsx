// C9-C13: the journey's views as one menu in its header, "View: Graph ▾": the Graph, List,
// Timeline, Summary and the Walkthrough, each a link, the one open marked as where you are.
// Picking a view is navigation, so the items are links with real addresses; the view state
// every view shares goes with the pick (journeys/view.ts).
import { Link, useLocation } from "react-router";

import { SCREENS, SCREEN_WORDS, screenSwitched, type Screen } from "../journeys/address.ts";
import { useJourneyTarget } from "../journeys/useJourneyTarget.ts";
import { Menu } from "./Menu.tsx";

export function ViewPicker({ screen }: { screen: Screen }) {
  const { search } = useLocation();
  const target = useJourneyTarget();
  return (
    <Menu
      label={`View: ${SCREEN_WORDS[screen]}`}
      title="Which view of the journey. v cycles the Graph, List and Timeline; w opens the Walkthrough"
      testId="view-picker"
      align="end"
      className="no-print view-picker"
      trigger={<>View: {SCREEN_WORDS[screen]} ▾</>}
    >
      {(close) =>
        SCREENS.map((each) => (
          <Link
            key={each}
            role="menuitem"
            className="menu-item"
            aria-current={each === screen ? "page" : undefined}
            data-testid={`view-${each}`}
            to={screenSwitched(target, search, each)}
            onClick={close}
          >
            {SCREEN_WORDS[each]}
          </Link>
        ))
      }
    </Menu>
  );
}
