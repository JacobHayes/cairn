// The routes of a journey's addresses (journeys/address.ts): its views, the Summary, the walk
// through, the landing, and the deep link to a node. Anything else lands on the shell's "no such
// page".
import { Link, Navigate, useParams } from "react-router";

import { ancestorsOf, placedAt } from "../canvas/ladder.ts";
import { canvasPath, DEFAULT_GRAPH_VIEW, revealing } from "../canvas/settings.ts";
import { nodeOf } from "../detail/model.ts";
import { DEFAULT_PROJECTION, pagePath, type Screen } from "../journeys/address.ts";
import { JourneyFrame, JourneyGate } from "./JourneyFrame.tsx";

/** `/journeys/<id>/{graph|list|timeline|summary|walk}[/nodes/<key>|/edges/<from>~<to>]`. */
export function JourneyPageRoute({ screen }: { screen: Screen }) {
  const { id = "", key, edge } = useParams();
  return <JourneyFrame key={id} id={id} screen={screen} selected={key} edge={edge} />;
}

/** `/journeys/<id>`: the landing, always the graph. */
export function JourneyLanding() {
  const { id = "" } = useParams();
  return <Navigate replace to={pagePath(id, DEFAULT_PROJECTION)} />;
}

/**
 * `/journeys/<id>/nodes/<key>`, the address agents post: the graph, with the node selected and
 * centred and its containers open.
 */
export function JourneyDeepLink() {
  const { id = "", key = "" } = useParams();
  return (
    <JourneyGate id={id}>
      {(ready) => {
        const found = nodeOf(ready, key);
        const reveal = found === undefined ? DEFAULT_GRAPH_VIEW : revealing(DEFAULT_GRAPH_VIEW, placedAt(ready.journey.graph.nodes ?? [], found.kind, ancestorsOf(ready.journey.graph.nodes ?? [], key)), ready.derived.nodes[key]?.display_state);
        return <Navigate replace to={canvasPath(id, reveal, key)} state={{ reveal: { key, how: "centre" } }} />;
      }}
    </JourneyGate>
  );
}

/** An address that is none of the app's: the shell's "no such page". */
export function NoSuchPage() {
  return (
    <p className="callout" data-testid="no-such-page">
      There is no such page. <Link to="/journeys">All journeys</Link>
    </p>
  );
}
