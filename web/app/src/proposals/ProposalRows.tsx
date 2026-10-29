// Open proposals as rows (C14, C16): one row per proposal, its title a link to its review under
// its destination and one fact under it. The Proposals tab of a journey or route, Home and the
// Journeys and Routes indexes all draw them with `ProposalRow` and let `RowsPanel` bound them.
import type { ReactNode } from "react";
import { Link, useLocation } from "react-router";

import { proposalAddress } from "../address.ts";
import type { PanelRow } from "../acting/RowsPanel.tsx";
import type { ProposalSummary } from "../data/proposals.ts";
import { outlineWords } from "./words.ts";

/**
 * One proposal's row: its title, and under it what it changes (the outline), or that it needs a
 * refresh. `context` names the destination where the list does not (Home, the indexes); `current`
 * marks the one whose layer is on. The link keeps the page's view.
 */
export function ProposalRow({ item, context, current }: { item: ProposalSummary; context?: ReactNode; current?: boolean }) {
  const { search } = useLocation();
  const stale = item.stale;
  return (
    <li className="node-row proposal-row" data-row data-testid="proposal-row" data-proposal={item.id} data-stale={stale} aria-current={current === true ? "true" : undefined} data-selected={current === true}>
      <div className="node-row-main">
        <span className="node-row-text">
          <Link to={{ pathname: proposalAddress(item.destination, item.id), search }} className="node-title" data-row-title>
            {item.title}
          </Link>
          <span className="node-fact muted" data-testid="fact">
            {context === undefined ? null : <>{context} &middot; </>}
            {outlineWords(item.outline)}
            {stale ? <> <span className="ink-warn" data-testid="needs-refresh">Needs a refresh</span></> : null}
          </span>
        </span>
      </div>
    </li>
  );
}

/** The rows of `items`, for a `RowsPanel`. */
export function proposalRows(items: readonly ProposalSummary[], context?: (item: ProposalSummary) => ReactNode, current?: string): PanelRow[] {
  return items.map((item) => ({ key: item.id, render: () => <ProposalRow item={item} context={context?.(item)} current={item.id === current} /> }));
}

/** The destination as a row names it where the list does not: its name, the identities for the deployment, or the one a proposal would create. */
export function destinationWords(item: ProposalSummary): string {
  const { destination, now } = item;
  if (destination === "deployment") {
    return "Identities";
  }
  if (now.exists && now.name != null) {
    return now.name;
  }
  if ("route" in destination) {
    return `New route ${destination.route}`;
  }
  return "New journey";
}
