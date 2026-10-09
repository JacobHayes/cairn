// `/`, the landing (2.1): Mine when the viewer has at least one item to act on now in an
// active journey (participants are the primary users, and Mine is their "what do I do now"),
// and the journey index otherwise. Which it is depends on every candidate journey's own
// "mine", derived in the tab, so the landing waits for them and then moves on.
import { useEffect } from "react";
import { Navigate, useLocation } from "react-router";

import { indexKey, journeyIndex, type JourneySummary } from "../data/reads.ts";
import { useLive, useViewer } from "../data/react.ts";
import { DEFAULT_FILTERS, indexPath, legacyRedirect, MINE_PATH, queryOf } from "./address.ts";
import { useMineCounts, useMineOf } from "./mine.ts";

/** Reports how many of journey `id`'s "mine" are on its acting frontier; none when it cannot be read. */
function Probe({ id, report }: { id: string; report: (id: string, count: number) => void }) {
  const mine = useMineOf(id);
  const count =
    mine.status === "ready" ? mine.entries.filter((entry) => mine.view.derived.acting_frontier.includes(entry.node)).length : mine.status === "loading" ? undefined : 0;
  useEffect(() => {
    if (count !== undefined) {
      report(id, count);
    }
  }, [id, count, report]);
  return null;
}

function Settling({ items }: { items: JourneySummary[] }) {
  const { report, settled, total } = useMineCounts(items);
  if (settled) {
    return <Navigate replace to={total > 0 ? MINE_PATH : indexPath()} />;
  }
  return (
    <>
      <p className="muted small">Reading your journeys...</p>
      {items.map((item) => (
        <Probe key={item.id} id={item.id} report={report} />
      ))}
    </>
  );
}

export function Home() {
  const { pathname, search } = useLocation();
  const { viewer, failed } = useViewer();
  const query = viewer === undefined ? undefined : queryOf({ ...DEFAULT_FILTERS, mine: true }, viewer.entities);
  const { view } = useLive(indexKey(query), journeyIndex(query));
  // The index was here once, with its filters in the address.
  const moved = legacyRedirect(pathname, search);
  if (moved !== undefined) {
    return <Navigate replace to={moved} />;
  }
  if (failed !== undefined || (viewer !== undefined && query === undefined) || view.status === "failed" || view.status === "missing") {
    return <Navigate replace to={indexPath()} />;
  }
  if (viewer === undefined || view.status === "loading") {
    return <p className="muted small">Reading your journeys...</p>;
  }
  return view.value.length === 0 ? <Navigate replace to={indexPath()} /> : <Settling items={view.value} />;
}
