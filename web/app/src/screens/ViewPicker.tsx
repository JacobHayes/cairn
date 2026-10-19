// C9-C13: the views of a journey or a route as one menu in its header, "View: Graph ▾": each a
// link, the one open marked as where you are. A journey offers the Graph, List, Timeline, Summary
// and the Walkthrough; a route the first three. Picking a view is navigation, so the items are
// links with real addresses; the view state every view shares goes with the pick
// (journeys/view.ts).
import { Link } from "react-router";

import { SCREEN_WORDS, type Screen } from "../journeys/address.ts";
import { Menu } from "./Menu.tsx";

export interface ViewPickerProps {
  screen: Screen;
  /** The views on offer, in order. */
  screens: readonly Screen[];
  /** The address of view `to` from the page as it is now. */
  to: (to: Screen) => string;
  /** What the menu's tooltip says: which view of what, and its keys. */
  title: string;
}

export function ViewPicker({ screen, screens, to, title }: ViewPickerProps) {
  return (
    <Menu label={`View: ${SCREEN_WORDS[screen]}`} title={title} testId="view-picker" align="end" className="no-print view-picker" trigger={<><span className="phone-hidden">View: </span>{SCREEN_WORDS[screen]} ▾</>}>
      {(close) =>
        screens.map((each) => (
          <Link key={each} role="menuitem" className="menu-item" aria-current={each === screen ? "page" : undefined} data-testid={`view-${each}`} to={to(each)} onClick={close}>
            {SCREEN_WORDS[each]}
          </Link>
        ))
      }
    </Menu>
  );
}

/** The tooltip of a journey's View picker. */
export const JOURNEY_VIEW_TITLE = "Which view of the journey. v cycles the Graph, List and Timeline; w opens the Walkthrough";

/** The tooltip of a route's View picker. */
export const ROUTE_VIEW_TITLE = "Which view of the route. v cycles the Graph, List and Timeline";
