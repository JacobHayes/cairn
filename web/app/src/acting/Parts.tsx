// What a row on the acting surfaces shows besides its actions: a link to the node's detail
// beside the screen, its breadcrumb (C9), and its derived flags (D3).
import { Link, useLocation } from "react-router";

import { startedEarly, titleOf, type Ready } from "../detail/model.ts";
import { nodePath, screenPath } from "../detail/parts.tsx";
import { Badge, type Tone } from "../ui/kit.tsx";
import type { NodeRow } from "./why.ts";

/** Node `key`'s detail in the inspector beside the screen it is on, keeping what the screen shows (5.1). */
export function DetailLink({ view, node, className }: { view: Ready; node: string; className?: string }) {
  const { pathname, search } = useLocation();
  return (
    <Link to={{ pathname: nodePath(screenPath(pathname), node), search }} data-node={node} {...(className === undefined ? {} : { className })}>
      {titleOf(view, node)}
    </Link>
  );
}

/** C9: the row's ancestors, root first, each opening its detail. */
export function Crumb({ view, row }: { view: Ready; row: NodeRow }) {
  const ancestors = row.ancestors ?? [];
  if (ancestors.length === 0) {
    return <span className="muted small crumb" data-testid="breadcrumb">Top of the journey</span>;
  }
  return (
    <span className="muted small crumb" data-testid="breadcrumb">
      {ancestors.map((key, at) => (
        <span key={key}>
          {at === 0 ? "" : " / "}
          <DetailLink view={view} node={key} />
        </span>
      ))}
    </span>
  );
}

/** D3: the row's flags that are set; what its state chip already says (blocked, snoozed, conditional) is not repeated (D8). */
export function Flags({ view, row }: { view: Ready; row: NodeRow }) {
  const derived = view.derived.nodes[row.key];
  const flags: [boolean | undefined, string, Tone][] = [
    [derived !== undefined && startedEarly(derived, row.state), "started early", "plain"],
    [row.unassigned, "unassigned", "warn"],
    [row.overdue, "overdue", "bad"],
    [row.shortfall_days != null, "shortfall", "bad"],
    [row.stale, "stale", "warn"],
    [row.needs_breakdown, "needs breakdown", "warn"],
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
