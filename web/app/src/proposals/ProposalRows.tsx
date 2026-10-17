// Open proposals as rows (C14, C16): one row per proposal, its title a link to its review under
// its destination and one fact under it. The Proposals tab of a journey or route, Home and the
// Journeys and Routes indexes all draw them with `ProposalRow` and let `RowsPanel` bound them.
import type { ReactNode } from "react";
import { Link } from "react-router";

import { proposalAddress } from "../address.ts";
import type { PanelRow } from "../acting/RowsPanel.tsx";
import { useUserWords } from "../data/react.ts";
import type { ProposalSummary } from "../data/proposals.ts";

/** Whether the destination moved since the proposal was drafted (a create whose destination now exists counts), so it needs a refresh before it can be applied (I6). */
export function needsRefresh(item: ProposalSummary): boolean {
  return item.now.revision !== item.drafted_against;
}

/** One proposal's row: its title, and under it who proposed it, or that it needs a refresh. `context` names the destination where the list does not (Home, the indexes). */
export function ProposalRow({ item, context }: { item: ProposalSummary; context?: ReactNode }) {
  const userWords = useUserWords();
  const stale = needsRefresh(item);
  const who = `Proposed by ${userWords(item.created_by)}${item.proposing_agent == null ? "" : ` with the agent ${item.proposing_agent}`}`;
  return (
    <li className="node-row proposal-row" data-row data-testid="proposal-row" data-proposal={item.id} data-stale={stale}>
      <div className="node-row-main">
        <span className="node-row-text">
          <Link to={proposalAddress(item.destination, item.id)} className="node-title" data-row-title>
            {item.title}
          </Link>
          <span className="node-fact muted" data-testid="fact">
            {context === undefined ? null : <>{context} &middot; </>}
            {stale ? <span className="node-fact-warn" data-testid="needs-refresh">Needs a refresh</span> : who}
          </span>
        </span>
      </div>
    </li>
  );
}

/** The rows of `items`, for a `RowsPanel`. */
export function proposalRows(items: readonly ProposalSummary[], context?: (item: ProposalSummary) => ReactNode): PanelRow[] {
  return items.map((item) => ({ key: item.id, render: () => <ProposalRow item={item} context={context?.(item)} /> }));
}

/** The destination as a row names it where the list does not: its name, the entities for the deployment, or the one a proposal would create. */
export function destinationWords(item: ProposalSummary): string {
  const { destination, now } = item;
  if (destination === "deployment") {
    return "Entities";
  }
  if (now.exists && now.name != null) {
    return now.name;
  }
  if ("route" in destination) {
    return `New ${now.kind === "segment" ? "segment" : "route"} ${destination.route}`;
  }
  return "New journey";
}
