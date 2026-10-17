// The routes of a journey's addresses (journeys/address.ts): `/journeys/<id>`, a node in it
// (`/nodes/<key>`) or an edge (`/edges/<from>~<to>`), all one page; the view is its `view=`
// query, defaulting by what the path names. Anything else lands on the shell's "no such page".
import { useMemo } from "react";
import { Link, useLocation } from "react-router";

import { indexPath, screenOf } from "../journeys/address.ts";
import { useJourneyTarget } from "../journeys/useJourneyTarget.ts";
import { JourneyFrame } from "./JourneyFrame.tsx";

/** A journey's page: its address, a node in it or an edge (address.ts), `?view=...`. */
export function JourneyPage() {
  const target = useJourneyTarget();
  const { search } = useLocation();
  const screen = useMemo(() => screenOf(target, new URLSearchParams(search)), [target, search]);
  return <JourneyFrame key={target.journey} id={target.journey} screen={screen} selected={target.node} edge={target.edge} />;
}

/** An address that is none of the app's: the shell's "no such page". */
export function NoSuchPage() {
  return (
    <p className="callout" data-testid="no-such-page">
      There is no such page. <Link to={indexPath()}>All journeys</Link>
    </p>
  );
}
