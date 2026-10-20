// The small pieces every node detail section is built from: a collapsible section (C8 shows
// everything, so most of it starts folded: progressive disclosure), a link to another node's
// detail, and a list of contributing nodes.
import { createContext, use, type ReactNode } from "react";
import { Link } from "react-router";

import "./detail.css";
import { useNodeAddresses } from "../journeys/address.ts";
import { Markdown } from "../ui/markdown.tsx";
import { setFold, useFold } from "./folds.ts";
import { titleOf, type Ready } from "./model.ts";
import { assignOwner } from "../acting/acts.ts";
import { entityName } from "./sections.tsx";
import type { NodeWrite } from "./write.ts";

/** B2: an answer's rationale as the journey records it, rendered as markdown. */
export function Rationale({ text }: { text: string | undefined }) {
  return text === undefined ? null : (
    <div className="chain stack" data-testid="rationale">
      <span className="muted small">Why</span>
      <Markdown text={text} />
    </div>
  );
}

/** A link to another node's detail, by its title, keeping the view and what it shows (5.2). */
export function NodeLink({ view, node }: { view: Ready; node: string }) {
  const addresses = useNodeAddresses();
  return (
    <Link to={addresses.open(node)} data-node={node}>
      {titleOf(view, node)}
    </Link>
  );
}

/** Whether the panel starts every section folded, as a card does; `Section`'s `keep` opts one out. */
export const FoldedSections = createContext(false);

/**
 * One section of the panel: its heading, a one-line summary always shown, and the rest
 * behind it, open when `open` (the sections a person acts on first). A folded panel opens only
 * the sections that `keep` open, the ones its form needs in view. With a `fold` key the
 * viewer's own choice is remembered and wins over `open` (folds.ts), except in a folded panel.
 */
export function Section({
  title,
  summary,
  open = false,
  keep = false,
  testId,
  fold,
  children,
}: {
  title: string;
  summary?: ReactNode;
  open?: boolean;
  keep?: boolean;
  testId: string;
  fold?: string;
  children?: ReactNode;
}) {
  const folded = use(FoldedSections) && !keep;
  // A card starts calm whatever the inspector remembers, and its own toggles are not remembered.
  const remembered = folded ? undefined : fold;
  const shown = useFold(remembered, open && !folded);
  return (
    <details
      className="detail-section"
      open={shown}
      data-testid={testId}
      onToggle={(event) => {
        // Only a person's click is a choice: setting `open` here fires a toggle too.
        if (remembered !== undefined && event.currentTarget.open !== shown) {
          setFold(remembered, event.currentTarget.open);
        }
      }}
    >
      <summary>
        <span className="detail-section-title">{title}</span>
        {summary === undefined || summary === "0" ? null : <span className="muted small"> {summary}</span>}
      </summary>
      <div className="stack detail-section-body">{children}</div>
    </details>
  );
}

/** Contributing nodes and what each adds (Priority: gravity and unlocks). */
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
    return <span className="muted small">None.</span>;
  }
  return (
    <ul className="detail-list" data-testid={testId}>
      {entries.map((entry) => (
        <li key={entry.node}>
          <NodeLink view={view} node={entry.node} /> <span className="muted small">+{entry.score.toFixed(2)}</span>
        </li>
      ))}
    </ul>
  );
}

/** E6: the people the deployment lists, as a select's options, each by its name through any merge. */
export function EntityOptions({ view }: { view: Ready }) {
  return (view.inputs.deployment.entities ?? []).map((each) => (
    <option key={each.key} value={each.key}>
      {entityName(view, each.key)}
    </option>
  ));
}

/** D2, E1: make a person the node's owner by choosing them: the choice sends it. */
export function OwnerSelect({ view, write, node, placeholder, className, testId }: { view: Ready; write: NodeWrite; node: string; placeholder: string; className?: string; testId?: string }) {
  return (
    <select className={className} aria-label="Assign owner" data-testid={testId} disabled={write.disabled} value="" onChange={(event) => { void write.run([assignOwner(node, event.target.value)]); }}>
      <option value="">{placeholder}</option>
      <EntityOptions view={view} />
    </select>
  );
}
