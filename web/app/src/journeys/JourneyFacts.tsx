// What the journey card and the Summary both say about a journey: the flags that need a look,
// as links into the plan list, and its lineage (the route and version it started from). One module, so the inspector's card and the front page cannot drift apart.
import { Link } from "react-router";

import { valueOf } from "../data/keyed-read.ts";
import { useCaller, useLive } from "../data/react.ts";
import { routeIndex } from "../data/reads.ts";
import type { Ready } from "../detail/model.ts";
import { identityNameList, NO_OWNER } from "../detail/sections.tsx";
import { routePath } from "../routes/address.ts";
import type { SummaryModel } from "../summary/model.ts";
import { dateWords } from "../status/when.ts";
import { TROUBLES } from "../status/words.ts";
import { namedList } from "../ui/words.ts";
import { journeyPath } from "./address.ts";
import { listPath } from "./view.ts";

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

/** The most owners the card names before "and N more". */
const OWNERS_SHOWN = 3;

/**
 * C16: who owns the journey, as "Owned by you and Ana Silva", or "No owner": on the card at most
 * three names, the rest as "and N more" linking to the Summary, which names them all (`all`).
 */
export function OwnersLine({ ready, all = false }: { ready: Ready; all?: boolean }) {
  const { caller } = useCaller();
  const { header } = ready.journey;
  const names = identityNameList(ready.inputs.deployment, header.owners ?? [], caller?.identities ?? [], "you");
  if (names.length === 0) {
    return (
      <span className="muted" data-testid="journey-owners" data-owners="0">
        {NO_OWNER}
      </span>
    );
  }
  const shown = all || names.length <= OWNERS_SHOWN ? names : names.slice(0, OWNERS_SHOWN - 1);
  const more = names.length - shown.length;
  const named = more === 0 ? namedList(shown, Infinity) : shown.join(", ");
  return (
    <span data-testid="journey-owners" data-owners={names.length}>
      Owned by {named}
      {more === 0 ? null : (
        <>
          {" and "}
          <Link to={journeyPath({ journey: header.id }, "summary")}>{more} more</Link>
        </>
      )}
    </span>
  );
}

/** The route and version a journey started from, and whether the route has a newer one (C16). */
export interface Lineage {
  route: string;
  version: number;
  name: string;
}

/** C16: the journey's lineage, the route and version it was made from; none for a journey that did not come from a route. */
export function useLineage(ready: Ready): Lineage | undefined {
  const lineage = ready.journey.header.lineage ?? undefined;
  const routes = useLive("routes", routeIndex).view;
  if (lineage === undefined) {
    return undefined;
  }
  const route = valueOf(routes)?.find((each) => each.header.id === lineage.route);
  return { route: lineage.route, version: lineage.version, name: route?.header.name ?? lineage.route };
}

/**
 * C16: where the journey came from (the route and version, its graph) and the day it started.
 */
export function LineageBlock({ ready, lineage }: { ready: Ready; lineage: Lineage | undefined }) {
  const { header } = ready.journey;
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
      </span>
      {started}
    </>
  );
}

