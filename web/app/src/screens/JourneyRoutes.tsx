// The routes of a journey's addresses (journeys/address.ts): its views, the Summary, the walk
// through, the landing, the deep link to a node, and every address the earlier screens had,
// which resolves to its new place. Anything else lands on the shell's "no such page".
import { Link, Navigate, useLocation, useParams } from "react-router";

import { ancestorsOf } from "../canvas/ladder.ts";
import { canvasPath, DEFAULT_VIEW, revealing } from "../canvas/settings.ts";
import { nodeOf } from "../detail/model.ts";
import { legacyRedirect, pagePath, type Screen } from "../journeys/address.ts";
import { keptQuery } from "../journeys/kept.ts";
import { recalledProjection } from "../journeys/memory.ts";
import { JourneyFrame, JourneyGate } from "./JourneyFrame.tsx";

/** `/journeys/<id>/{graph|list|timeline|summary|walk}[/nodes/<key>|/edges/<from>~<to>]`. */
export function JourneyPageRoute({ screen }: { screen: Screen }) {
  const { id = "", key, edge } = useParams();
  return <JourneyFrame key={id} id={id} screen={screen} selected={key} edge={edge} />;
}

/**
 * `/journeys/<id>`: the landing, the view the viewer last had of the journey (the graph at
 * first), or the graph for the old canvas's address with its own query.
 */
export function JourneyLanding() {
  const { id = "" } = useParams();
  const { pathname, search } = useLocation();
  const projection = recalledProjection(id);
  return <Navigate replace to={legacyRedirect(pathname, search) ?? pagePath(id, projection, undefined, keptQuery(id, projection))} />;
}

/**
 * `/journeys/<id>/nodes/<key>`, the address agents post: the graph, with the node selected and
 * centred and its containers open.
 */
export function JourneyDeepLink() {
  const { id = "", key = "" } = useParams();
  const { pathname, search } = useLocation();
  const old = legacyRedirect(pathname, search);
  if (old !== undefined) {
    return <Navigate replace to={old} />;
  }
  return (
    <JourneyGate id={id}>
      {(ready) => {
        const found = nodeOf(ready, key);
        const reveal = found === undefined ? DEFAULT_VIEW : revealing(DEFAULT_VIEW, { kind: found.kind, ancestors: ancestorsOf(ready.journey.graph.nodes ?? [], key) }, ready.derived.nodes[key]?.display_state);
        return <Navigate replace to={canvasPath(id, reveal, key)} state={{ reveal: { key, how: "centre" } }} />;
      }}
    </JourneyGate>
  );
}

/** An address that no longer exists, resolved to its new place, or the shell's "no such page". */
export function NoSuchPage() {
  const { pathname, search } = useLocation();
  const to = legacyRedirect(pathname, search);
  return to === undefined ? (
    <p className="callout" data-testid="no-such-page">
      There is no such page. <Link to="/journeys">All journeys</Link>
    </p>
  ) : (
    <Navigate replace to={to} />
  );
}

/** An earlier address of a draft node's form, `/routes/<id>/nodes/<key>`, opens it at the draft's. */
export function RouteNodeRedirect() {
  const { id = "", key = "" } = useParams();
  const { search } = useLocation();
  return <Navigate replace to={`/routes/${id}/draft/nodes/${key}${search}`} />;
}
