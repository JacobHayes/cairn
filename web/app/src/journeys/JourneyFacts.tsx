// What the journey card and the Summary both say about a journey: the flags that need a look,
// as links into the plan list, and its lineage (the route and version it started from, its
// upgrade). One module, so the inspector's card and the front page cannot drift apart.
import { useState } from "react";
import { Link } from "react-router";

import type { ListFlag } from "../acting/address.ts";
import { valueOf } from "../data/keyed-read.ts";
import { useLive } from "../data/react.ts";
import { indexKey, journeyIndex, routeIndex } from "../data/reads.ts";
import type { Ready } from "../detail/model.ts";
import { JourneyFlow } from "../proposals/Entries.tsx";
import { Badge } from "../ui/kit.tsx";
import { routeDetailPath } from "../routes/address.ts";
import type { SummaryModel } from "../summary/model.ts";
import { dateWords } from "../timeline/model.ts";
import { upgradeWords } from "./figures-model.ts";
import { listPath } from "./view.ts";

const nodes = (count: number) => `${String(count)} ${count === 1 ? "node" : "nodes"}`;

/** What a flag is called in a sentence (`words`, on the journey card), how the Summary says it of `count` nodes, and its ink there (D3, D4, F6). */
const FLAGS: { flag: ListFlag; words: string; sentence: (count: number) => string; tone: "bad" | "warn"; count: (model: SummaryModel) => number }[] = [
  { flag: "overdue", words: "overdue", sentence: (count) => `${nodes(count)} overdue`, tone: "bad", count: (model) => model.overdue.length },
  { flag: "shortfall", words: "short of days", sentence: (count) => `${nodes(count)} without the days they need`, tone: "bad", count: (model) => model.shortfalls.length },
  { flag: "stale", words: "stale", sentence: (count) => `${nodes(count)} stale`, tone: "warn", count: (model) => model.stale.length },
];

/**
 * What needs a look, as one short sentence of links into the plan list: only what is not zero.
 * `toned` (the Summary's) says overdue and short in error ink and stale in warning ink, always
 * with the words.
 */
export function Flags({ ready, model, toned = false }: { ready: Ready; model: SummaryModel; toned?: boolean }) {
  const journey = ready.journey.header.id;
  const flags = FLAGS.map((each) => ({ ...each, count: each.count(model) })).filter((each) => each.count > 0);
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
            className={toned ? `flag-${each.tone}` : undefined}
            data-testid={`card-flag-${each.flag}`}
          >
            {toned ? each.sentence(each.count) : `${String(each.count)} ${each.words}`}
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
 * with the upgrade offered when the route has moved on, unless `readOnly` (an archived journey
 * accepts only un-archiving or deletion, A19).
 */
export function LineageBlock({ ready, lineage, readOnly = false }: { ready: Ready; lineage: Lineage | undefined; readOnly?: boolean }) {
  const [upgrading, setUpgrading] = useState(false);
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
        <Link to={routeDetailPath(lineage.route)}>
          {lineage.name}, version {lineage.version}
        </Link>
        <Link to={`/routes/${lineage.route}?version=${String(lineage.version)}`} className="no-print" data-testid="lineage">
          See its graph
        </Link>
        {summary === undefined || upgradeWords(summary) === undefined ? null : (
          <Badge tone="warn" data-testid="upgrade" data-status="available">
            {upgradeWords(summary)}
          </Badge>
        )}
        {lineage.latest === undefined || readOnly ? null : (
          <button type="button" className="chip" data-testid="journey-upgrade" title="Propose this journey's upgrade to the route's newer version" onClick={() => { setUpgrading(true); }}>
            Upgrade to v{lineage.latest}
          </button>
        )}
      </span>
      {upgrading && !readOnly ? <JourneyFlow ready={ready} flow="upgrade" onClose={() => { setUpgrading(false); }} /> : null}
      {started}
    </>
  );
}

