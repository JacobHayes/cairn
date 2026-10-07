// What a row or card on the acting surfaces shows besides its actions: a link to the node's
// detail on the canvas, its breadcrumb (C10), its derived flags (D3), and why it ranks where
// it does (C10).
import { Link } from "react-router";

import { canvasPath, DEFAULT_VIEW } from "../canvas/settings.ts";
import { nodeOf, titleOf, type Ready } from "../detail/model.ts";
import { Badge, type Tone } from "../ui/kit.tsx";
import type { SortBy } from "./address.ts";
import { whyWords, type NodeRow } from "./why.ts";

/** Node `key`'s detail beside the journey's canvas (5.1, 5.2), at the canvas's defaults. */
export function DetailLink({ view, node, className }: { view: Ready; node: string; className?: string }) {
  return (
    <Link to={canvasPath(view.journey.header.id, DEFAULT_VIEW, node)} data-node={node} {...(className === undefined ? {} : { className })}>
      {titleOf(view, node)}
    </Link>
  );
}

/** C10: the row's ancestors, root first, each opening its detail. */
export function Crumb({ view, row }: { view: Ready; row: NodeRow }) {
  const ancestors = row.ancestors ?? [];
  if (ancestors.length === 0) {
    return <span className="muted crumb" data-testid="breadcrumb">Top of the journey</span>;
  }
  return (
    <span className="muted crumb" data-testid="breadcrumb">
      {ancestors.map((key, at) => (
        <span key={key}>
          {at === 0 ? "" : " / "}
          <DetailLink view={view} node={key} />
        </span>
      ))}
    </span>
  );
}

/** D3: the row's flags that are set. */
export function Flags({ row }: { row: NodeRow }) {
  const flags: [boolean | undefined, string, Tone][] = [
    [row.unassigned, "unassigned", "warn"],
    [row.overdue, "overdue", "bad"],
    [row.shortfall_days != null, "shortfall", "bad"],
    [row.blocked, "blocked", "warn"],
    [row.snoozed != null, "snoozed", "plain"],
    [row.stale, "stale", "warn"],
    [row.needs_breakdown, "needs breakdown", "warn"],
    [row.relevance === "undecided", "undecided", "plain"],
  ];
  return (
    <span className="row">
      {flags
        .filter(([set]) => set === true)
        .map(([, flag, tone]) => (
          <Badge key={flag} tone={tone} data-testid="flag" data-status={flag}>
            {flag}
          </Badge>
        ))}
    </span>
  );
}

/** C10: why the row ranks where it does under `sort`, the blend's parts in words. */
export function Why({ view, row, sort }: { view: Ready; row: NodeRow; sort: SortBy }) {
  const estimate = nodeOf(view, row.key)?.estimate ?? undefined;
  return (
    <span className="muted" data-testid="why" data-rank={row.rank?.rank}>
      Why here: {whyWords(row, view.inputs.rank, sort, estimate)}
    </span>
  );
}
