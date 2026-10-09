// The routes of a journey's addresses (journeys/address.ts): its pages, the Summary page, the
// landing, the deep link to a node, and every address the earlier screens had, which resolves
// to its new place (2.4). The landing and the deep link need the journey to resolve: where a
// journey opens depends on what is in it.
import type { ReactNode } from "react";
import { Navigate, useLocation, useParams } from "react-router";

import { nodeOf } from "../detail/model.ts";
import { deepLinkTarget, landingPath, legacyRedirect, pagePath, projectionOf, type JourneyPage } from "../journeys/address.ts";
import { recalledProjection } from "../journeys/memory.ts";
import { JourneyFrame, JourneyGate } from "./JourneyFrame.tsx";

/** `/journeys/<id>/{next|plan}/<projection>[/nodes/<key>]`; a projection the page lacks opens the one it last had. */
export function JourneyPageRoute({ page }: { page: JourneyPage }) {
  const { id = "", projection, key } = useParams();
  const { search } = useLocation();
  const shown = projectionOf(page, projection);
  if (shown === undefined) {
    return <Navigate replace to={pagePath(id, page, recalledProjection(id, page), key, search)} />;
  }
  return <JourneyFrame key={id} id={id} page={page} projection={shown} selected={key} />;
}

/** The bare page address (`/journeys/<id>/plan`): the projection last shown there. */
export function BarePageRoute({ page }: { page: JourneyPage }) {
  const { id = "" } = useParams();
  const { search } = useLocation();
  return <Navigate replace to={pagePath(id, page, recalledProjection(id, page), undefined, search)} />;
}

/** C18: the Summary page, the journey card at full width. */
export function SummaryRoute() {
  const { id = "", key } = useParams();
  return <JourneyFrame key={id} id={id} page="summary" projection={undefined} selected={key} />;
}

/** An address of the earlier screens, or the old canvas, resolved to its new place (2.4, Redirects). */
function Resolved({ then }: { then: () => ReactNode }) {
  const { pathname, search } = useLocation();
  const to = legacyRedirect(pathname, search);
  return to === undefined ? then() : <Navigate replace to={to} />;
}

/**
 * `/journeys/<id>`: the landing. A journey just started, with a decision to make, opens on
 * the walkthrough (C11); any other opens on the next list (2.4).
 */
export function JourneyLanding() {
  const { id = "" } = useParams();
  return (
    <Resolved
      then={() => (
        <JourneyGate id={id}>
          {(ready) => (
            <Navigate
              replace
              to={landingPath(id, {
                recorded: ready.journey.revision > 1,
                decisions: ready.derived.acting_frontier.some((key) => nodeOf(ready, key)?.kind === "decision"),
              })}
            />
          )}
        </JourneyGate>
      )}
    />
  );
}

/**
 * `/journeys/<id>/nodes/<key>`, the address agents post: the node on the acting frontier opens
 * on NEXT, LIST; any other on PLAN, GRAPH, traced and with its container open (2.4).
 */
export function JourneyDeepLink() {
  const { id = "", key = "" } = useParams();
  return (
    <Resolved
      then={() => (
        <JourneyGate id={id}>
          {(ready) => (
            <Navigate
              replace
              to={deepLinkTarget(id, key, { onFrontier: ready.derived.acting_frontier.includes(key), parent: nodeOf(ready, key)?.parent ?? undefined })}
            />
          )}
        </JourneyGate>
      )}
    />
  );
}

/** An address of an earlier screen (`/journeys/<id>/list`, `/triage`, `/decisions`, ...). */
export function LegacyRoute() {
  const { pathname, search } = useLocation();
  const to = legacyRedirect(pathname, search);
  return to === undefined ? <p className="callout">There is no such page.</p> : <Navigate replace to={to} />;
}
