// The proposals that would create a journey or a route (C14, C16), on the index of that kind: a
// destination that does not exist yet has no page of its own to list them on, so this is where
// someone other than its author finds one. Present only while one is open.
import { RowsPanel } from "../acting/RowsPanel.tsx";
import { valueOf } from "../data/keyed-read.ts";
import { useLive } from "../data/react.ts";
import { Button } from "../ui/kit.tsx";
import { destinationWords, proposalRows } from "./ProposalRows.tsx";
import { listKey, proposalsListed } from "./read.ts";
import { Failed } from "../ui/Failed.tsx";

/** The open proposals that would create a journey (`new_journeys`) or a route (`new_routes`). */
export function NewDestinations({ scope }: { scope: "new_journeys" | "new_routes" }) {
  const { view, refresh } = useLive(listKey(scope), proposalsListed(scope));
  const found = valueOf(view);
  if (view.status === "failed") {
    return (
      <p className="callout callout-bad" role="alert" data-testid="proposed-failed">
        <Failed what="The proposals that would create one" error={view.error}>
          <Button onClick={refresh}>Try again</Button>
        </Failed>
      </p>
    );
  }
  if (found === undefined) {
    return <p className="muted small" data-testid="proposed-reading">Reading the proposals that would create one...</p>;
  }
  const { items } = found;
  return items.length === 0 ? null : <RowsPanel title="Proposed" testId="proposed-panel" rows={proposalRows(items, destinationWords)} />;
}
