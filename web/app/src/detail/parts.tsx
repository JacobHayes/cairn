// The small pieces every node detail section is built from: a collapsible section (C8 shows
// everything, so most of it starts folded: progressive disclosure), a link to another node's
// detail, and a list of contributing nodes.
import type { ReactNode } from "react";
import { Link, useLocation } from "react-router";

import { titleOf, type Ready } from "./model.ts";

/**
 * The address of the journey screen a node's detail is open on (the canvas, the timeline,
 * ...): `pathname` without its `/nodes/<key>`. Every screen opens the panel at its own address
 * with `/nodes/<key>` added, so the panel's links and its close stay on that screen.
 */
export function screenPath(pathname: string): string {
  return pathname.replace(/\/nodes\/[^/]*$/, "");
}

/** Node `key`'s detail on the journey screen at `screen` (`screenPath`). */
export function nodePath(screen: string, key: string): string {
  return `${screen}/nodes/${key}`;
}

/** A link to another node's detail, by its title, keeping the screen and what it shows (5.2). */
export function NodeLink({ view, node }: { view: Ready; node: string }) {
  const { pathname, search } = useLocation();
  return (
    <Link to={{ pathname: nodePath(screenPath(pathname), node), search }} data-node={node}>
      {titleOf(view, node)}
    </Link>
  );
}

/**
 * One section of the panel: its heading, a one-line summary always shown, and the rest
 * behind it, open when `open` (the sections a person acts on first).
 */
export function Section({
  title,
  summary,
  open = false,
  testId,
  children,
}: {
  title: string;
  summary?: ReactNode;
  open?: boolean;
  testId: string;
  children?: ReactNode;
}) {
  return (
    <details className="detail-section" open={open} data-testid={testId}>
      <summary>
        <span className="detail-section-title">{title}</span>
        {summary === undefined ? null : <span className="muted"> {summary}</span>}
      </summary>
      <div className="stack detail-section-body">{children}</div>
    </details>
  );
}

/** Contributing nodes and what each adds (Priority: gravity and leverage). */
export function Contributions({
  view,
  entries,
  testId,
}: {
  view: Ready;
  entries: { node: string; score: number; other_owner?: boolean }[];
  testId: string;
}) {
  if (entries.length === 0) {
    return <span className="muted">None.</span>;
  }
  return (
    <ul className="detail-list" data-testid={testId}>
      {entries.map((entry) => (
        <li key={entry.node}>
          <NodeLink view={view} node={entry.node} /> <span className="muted">+{entry.score.toFixed(2)}</span>
        </li>
      ))}
    </ul>
  );
}
