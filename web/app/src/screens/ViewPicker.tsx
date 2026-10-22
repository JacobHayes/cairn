// C9-C13: the views of a journey or a route as one menu in its header, "View: Graph ▾": each a
// link, the one open marked as where you are. A journey offers the Summary (where it opens), Graph, List, Timeline
// and the Walkthrough; a route the three projections. Picking a view is navigation, so the items are
// links with real addresses; the view state every view shares goes with the pick
// (journeys/view.ts).
import { Link } from "react-router";

import { SCREEN_WORDS, type Screen } from "../journeys/address.ts";
import { Menu } from "./Menu.tsx";
import { Tip } from "../ui/Tip.tsx";

export interface ViewPickerProps {
  screen: Screen;
  /** The views on offer, in order. */
  screens: readonly Screen[];
  /** The address of view `to` from the page as it is now. */
  to: (to: Screen) => string;
  /** What the menu's tooltip says: which view of what, and its keys. */
  tip: string;
  /** Views that are listed but cannot be shown now, each with its reason (DESIGN: a view that cannot honour the state says so). */
  unavailable?: Partial<Record<Screen, string>>;
}

export function ViewPicker({ screen, screens, to, tip, unavailable = {} }: ViewPickerProps) {
  return (
    <Menu label={`View: ${SCREEN_WORDS[screen]}`} tip={tip} testId="view-picker" align="end" className="no-print view-picker" trigger={<><span className="phone-hidden">View: </span>{SCREEN_WORDS[screen]} ▾</>}>
      {(close) =>
        screens.map((each) =>
          unavailable[each] !== undefined ? (
            <Tip key={each} tip={unavailable[each]} tap>
              <span role="menuitem" tabIndex={-1} className="menu-item" aria-disabled="true" data-testid={`view-${each}`}>
                {SCREEN_WORDS[each]}
              </span>
            </Tip>
          ) : (
          <Link key={each} role="menuitem" className="menu-item" aria-current={each === screen ? "page" : undefined} data-testid={`view-${each}`} to={to(each)} onClick={close}>
            {SCREEN_WORDS[each]}
          </Link>
          ),
        )
      }
    </Menu>
  );
}

/** The tooltip of a journey's View picker. */
export const JOURNEY_VIEW_TIP = "Which view of the journey. v cycles the Graph, List and Timeline; w opens the Walkthrough";

/** The tooltip of a route's View picker. */
export const ROUTE_VIEW_TIP = "Which view of the route. v cycles the Graph, List and Timeline";
