// The page's view state as the context value every view's controls read (journeys/view.ts): the
// state the address names, and how a control changes it, by navigating to the same page with
// the new state in its query. Shared by the journey's page and the route's.
import { useCallback, useMemo, useState } from "react";
import { useLocation, useNavigate } from "react-router";

import { withViewState, type ViewState, type ViewStateValue } from "../journeys/view.ts";

export function useViewStateValue(state: ViewState): ViewStateValue {
  const { pathname, search } = useLocation();
  const navigate = useNavigate();
  const [picks, setPicks] = useState(0);
  const onState = useCallback(
    (next: ViewState, picked = false) => {
      if (picked) {
        setPicks((count) => count + 1);
      }
      void navigate(`${pathname}${withViewState(search, next, picked)}`);
    },
    [navigate, pathname, search],
  );
  return useMemo<ViewStateValue>(() => ({ state, onState, picks }), [state, onState, picks]);
}
