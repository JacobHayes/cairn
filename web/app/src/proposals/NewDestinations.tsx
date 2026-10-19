// The proposals that would create a journey or a route (C14, C16), on the index of that kind: a
// destination that does not exist yet has no page of its own to list them on, so this is where
// someone other than its author finds one. Present only while one is open.
import { RowsPanel } from "../acting/RowsPanel.tsx";
import { valueOf } from "../data/keyed-read.ts";
import { useLive } from "../data/react.ts";
import { Button } from "../ui/kit.tsx";
import { destinationWords, proposalRows } from "./ProposalRows.tsx";
import { listKey, proposalsListed } from "./read.ts";
import { ReadFailed } from "../ui/ReadFailed.tsx";

/** The open proposals that would create a journey (`new_journeys`) or a route (`new_routes`). */
export function NewDestinations({ scope, segments }: { scope: "new_journeys" | "new_routes"; segments?: boolean | undefined }) {
  const { view, refresh } = useLive(listKey(scope), proposalsListed(scope));
  const found = valueOf(view);
  if (view.status === "failed") {
    return (
      <p className="callout callout-bad" role="alert" data-testid="proposed-failed">
        <ReadFailed what="The proposals that would create one" error={view.error} /> <Button onClick={refresh}>Try again</Button>
      </p>
    );
  }
  if (found === undefined) {
    return <p className="muted small" data-testid="proposed-reading">Reading the proposals that would create one...</p>;
  }
  // The Routes index shows only the kind it is showing: a segment proposal is not listed under Routes.
  const items = segments === undefined ? found.items : found.items.filter((item) => (item.now.kind === "segment") === segments);
  return items.length === 0 ? null : <RowsPanel title="Proposed" testId="proposed-panel" rows={proposalRows(items, destinationWords)} />;
}
