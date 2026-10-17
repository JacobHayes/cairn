// The Proposals tab of a journey's or a route's inspector column (C14): the open proposals under
// that destination, newest first, kept current as they are drafted, edited, applied or
// discarded. The tab is there while any is open, so a page with none keeps the column it would
// otherwise not need; its count is the page's, so the tab does not wait on the rows. The
// address `/journeys/<id>/proposals` (or the route's) opens the page with the tab in view even
// when none is open, which is where the `⋯` "Open proposals" and the review's way back go. A
// destination that does not exist yet has the tab only while a proposal would create it.
import { Link, useLocation } from "react-router";

import { proposalsAddress, usePage } from "../address.ts";
import { RowsPanel } from "../acting/RowsPanel.tsx";
import { valueOf } from "../data/keyed-read.ts";
import { useLive } from "../data/react.ts";
import type { Domain } from "../data/proposals.ts";
import { ProposalsSlot, useFrameState } from "../shell/frame.tsx";
import { Button } from "../ui/kit.tsx";
import { proposalRows } from "./ProposalRows.tsx";
import { listKey, proposalsUnder } from "./read.ts";

/** Whether the page is at its destination's `/proposals` address. */
function useAtProposals(): boolean {
  const name = usePage()?.name;
  return name === "journeyProposals" || name === "routeProposals";
}

/** The open proposals under `destination`: the Proposals tab and what it holds. */
export function DestinationProposals({ destination }: { destination: Domain }) {
  const { view, refresh } = useLive(listKey(destination), proposalsUnder(destination));
  const found = valueOf(view);
  const reveal = useAtProposals();
  // Nothing open and not asked for, or nothing open under a destination that does not exist (its
  // page says so): no tab, and no column held open for one.
  const none = (found?.count ?? 0) === 0 && view.status !== "failed";
  if (none && (!reveal || found?.destination?.exists === false)) {
    return null;
  }
  return (
    <ProposalsSlot count={found?.count} reveal={reveal}>
      <div className="stack" data-testid="proposals-pane" data-reading={found === undefined}>
        {found !== undefined ? (
          found.items.length === 0 ? (
            <None />
          ) : (
            <RowsPanel label="Open proposals" testId="proposals-panel" rows={proposalRows(found.items)} />
          )
        ) : view.status === "failed" ? (
          <p className="callout callout-bad" role="alert">
            The open proposals could not be read: {view.error.message}{" "}
            <Button onClick={refresh}>Try again</Button>
          </p>
        ) : view.status === "gone" ? (
          <None />
        ) : (
          <p className="muted small">Reading the open proposals...</p>
        )}
        {view.status === "behind" ? (
          <p className="muted small" data-testid="proposals-behind">
            These may be out of date: <Button ghost onClick={refresh}>Try again</Button>
          </p>
        ) : null}
      </div>
    </ProposalsSlot>
  );
}

function None() {
  return (
    <p className="muted small" data-testid="proposals-none">
      Nothing is waiting for review here. A proposal stays here from when it is drafted until it is applied or discarded.
    </p>
  );
}

/** The `⋯` entry that opens the destination's Proposals tab: always there, with the count in mono when there are any. */
export function OpenProposalsItem({ destination, close }: { destination: Domain; close: () => void }) {
  const count = useFrameState()?.proposalsCount;
  const { search } = useLocation();
  return (
    <Link role="menuitem" className="menu-item" data-testid="menu-proposals" to={{ pathname: proposalsAddress(destination), search }} onClick={close}>
      Open proposals
      {count === undefined || count === 0 ? null : <small className="mono">{count}</small>}
    </Link>
  );
}
