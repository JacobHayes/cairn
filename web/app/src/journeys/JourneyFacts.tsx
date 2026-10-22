// What the journey card and the Summary both say about a journey: the flags that need a look,
// as links into the plan list, and its lineage (the route and version it started from, its
// upgrade). One module, so the inspector's card and the front page cannot drift apart.
import { useState } from "react";
import { Link } from "react-router";

import { valueOf } from "../data/keyed-read.ts";
import { useLive } from "../data/react.ts";
import { indexKey, journeyIndex, routeIndex } from "../data/reads.ts";
import type { Ready } from "../detail/model.ts";
import { JourneyFlow } from "../proposals/Entries.tsx";
import { Badge } from "../ui/kit.tsx";
import { routePath } from "../routes/address.ts";
import { useWritable } from "../screens/writable.ts";
import type { SummaryModel } from "../summary/model.ts";
import { dateWords } from "../status/when.ts";
import { TROUBLES } from "../status/words.ts";
import { upgradeWords } from "./figures-model.ts";
import { listPath } from "./view.ts";
import { Tip } from "../ui/Tip.tsx";

/**
 * What needs a look, as one short sentence of links into the plan list: only what is not zero.
 * Overdue and short are said in error ink and stale in warning ink, always with the words.
 */
export function Flags({ ready, model }: { ready: Ready; model: SummaryModel }) {
  const journey = ready.journey.header.id;
  const flags = TROUBLES.map((each) => ({ ...each, count: each.count(model) })).filter((each) => each.count > 0);
  if (flags.length === 0) {
    return null;
  }
  return (
    <span data-testid="card-flags">
      {flags.map((each, at) => (
        <span key={each.flag}>
          {at === 0 ? "" : ", "}
          <Link
            to={listPath(journey, { flags: [each.flag] })}
            className={`ink-${each.tone}`}
            data-testid={`card-flag-${each.flag}`}
          >
            {each.words(each.count)}
          </Link>
        </span>
      ))}
    </span>
  );
}

/** The route and version a journey started from, and whether the route has a newer one (C16). */
export interface Lineage {
  route: string;
  version: number;
  name: string;
  /** The route's newest version, when it is newer than the journey's. */
  latest: number | undefined;
}

/** C16: the journey's lineage (the host's index gives its "upgrade available"); none for a journey that did not come from a route. */
export function useLineage(ready: Ready): Lineage | undefined {
  const lineage = ready.journey.header.lineage ?? undefined;
  const routes = useLive("routes", routeIndex).view;
  if (lineage === undefined) {
    return undefined;
  }
  const route = valueOf(routes)?.find((each) => each.header.id === lineage.route);
  const latest = route?.latest_version ?? undefined;
  return { route: lineage.route, version: lineage.version, name: route?.header.name ?? lineage.route, latest: latest !== undefined && latest > lineage.version ? latest : undefined };
}

/**
 * C16: where the journey came from (the route and version, its graph) and the day it started,
 * with the upgrade offered when the route has moved on and the page offers writes to the journey.
 */
export function LineageBlock({ ready, lineage }: { ready: Ready; lineage: Lineage | undefined }) {
  const [upgrading, setUpgrading] = useState(false);
  const writable = useWritable(ready);
  const { header } = ready.journey;
  const query = lineage === undefined ? undefined : { route: lineage.route, version: lineage.version };
  const { view } = useLive(indexKey(query), journeyIndex(query));
  const summary = valueOf(view)?.find((item) => item.id === header.id);
  const started = <span className="muted small">Started {dateWords(header.created_on, ready.derived.today)}</span>;
  if (lineage === undefined) {
    return started;
  }
  return (
    <>
      <span className="row" data-testid="overview-lineage">
        <Link to={routePath(lineage.route, { version: lineage.version })} data-testid="lineage">
          {lineage.name}, version {lineage.version}
        </Link>
        {summary === undefined || upgradeWords(summary) === undefined ? null : (
          <Badge tone="warn" data-testid="upgrade" data-status="available">
            {upgradeWords(summary)}
          </Badge>
        )}
        {lineage.latest === undefined || !writable ? null : (
          <Tip tip="Propose this journey's upgrade to the route's newer version">
            <button type="button" className="chip" data-testid="journey-upgrade" onClick={() => { setUpgrading(true); }}>
              Upgrade to v{lineage.latest}
            </button>
          </Tip>
        )}
      </span>
      {upgrading && writable ? <JourneyFlow ready={ready} flow="upgrade" onClose={() => { setUpgrading(false); }} /> : null}
      {started}
    </>
  );
}

