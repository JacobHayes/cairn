// The routes of a journey's addresses (journeys/address.ts): `/journeys/<id>`, a node in it
// (`/nodes/<key>`) or an edge (`/edges/<from>~<to>`), all one page; the view is its `view=`
// query, defaulting by what the path names. Anything else lands on the shell's "no such page".
import { useMemo } from "react";
import { Link, useLocation, useParams } from "react-router";

import { indexPath, screenOf } from "../journeys/address.ts";
import { JourneyFrame } from "./JourneyFrame.tsx";

/** `/journeys/<id>[/nodes/<key>|/edges/<from>~<to>]?view=...`. */
export function JourneyPage() {
  const { id = "", key, edge } = useParams();
  const { search } = useLocation();
  const screen = useMemo(() => screenOf({ journey: id, node: key, edge }, new URLSearchParams(search)), [id, key, edge, search]);
  return <JourneyFrame key={id} id={id} screen={screen} selected={key} edge={edge} />;
}

/** An address that is none of the app's: the shell's "no such page". */
export function NoSuchPage() {
  return (
    <p className="callout" data-testid="no-such-page">
      There is no such page. <Link to={indexPath()}>All journeys</Link>
    </p>
  );
}
