// Proposal review in the frame (design 4.10): the head (one line naming the proposal, how many
// items it holds and how many conflict, Apply as the primary and Discard under the `⋯`; the
// filter chips and the Graph and List switch), the List, and what the inspector shows: the item
// editor for the node picked (before and after, the choices it needs, its changes), or the
// proposal card while none is. Every kind of proposal lands here the same way.
import type { ReactNode } from "react";
import { Link } from "react-router";

import { Menu } from "../screens/Menu.tsx";
import { Badge, Button } from "../ui/kit.tsx";
import type { Graph } from "../authoring/graph.ts";
import { canvasPath, DEFAULT_VIEW } from "../canvas/settings.ts";
import { overviewPath } from "../journeys/address.ts";
import { routeDetailPath } from "../routes/address.ts";
import {
  DIFF_LABELS,
  filterOf,
  REVIEW_FILTERS,
  type Blocker,
  type DiffMark,
  type GraphDiff,
  type Proposal,
  type ReviewEntry,
  type ReviewFilter,
} from "./model.ts";
import { fieldName, fieldValueWords, type Names } from "./words.ts";

/** What each thing that stops an apply says, on Apply's hover and in the proposal card. */
export const BLOCKER_WORDS: Record<Blocker, string> = {
  not_open: "It is no longer open.",
  editing: "An edit has a problem: fix it or drop the change.",
  unsaved: "Save or drop your edits first.",
  stale: "Its destination moved: refresh it first.",
  unresolved: "An item still needs a choice.",
  invalid: "As it stands it breaks a rule; see the violations.",
  unreviewed: "Confirm you have reviewed it as it stands now.",
  no_preview: "Waiting for its preview.",
};

const FILTER_WORDS: Record<ReviewFilter, string> = { all: "All", conflicts: "Conflicts", add: "Add", change: "Change", remove: "Remove" };

/** Where the proposal goes, as a link, named the way the screens name it. */
function DestinationLink({ proposal, journeyTitle }: { proposal: Proposal; journeyTitle: string | undefined }) {
  const destination = proposal.destination;
  if (destination === "deployment") {
    return <span>the deployment</span>;
  }
  if ("journey" in destination) {
    return proposal.draft.destination_revision === 0 ? <span>a new journey</span> : <Link to={overviewPath(destination.journey)}>{journeyTitle ?? "the journey"}</Link>;
  }
  return <Link to={routeDetailPath(destination.route)}>the route {destination.route}</Link>;
}

/** Who the proposal is for and by whom, at the top of the proposal card. */
export function ProposalOrigin({ proposal, journeyTitle, names }: { proposal: Proposal; journeyTitle: string | undefined; names: Names }) {
  return (
    <p className="muted small">
      For <DestinationLink proposal={proposal} journeyTitle={journeyTitle} />, proposed by {names.entity(proposal.created_by)}
      {proposal.proposing_agent == null ? "" : ` with the agent ${proposal.proposing_agent}`}.
    </p>
  );
}

export interface ReviewHeaderProps {
  proposal: Proposal;
  itemCount: number;
  conflictCount: number;
  blockers: Blocker[];
  disabled: boolean;
  onApply: () => void;
  onDiscard: () => void;
}

/** One line: the proposal's name, its size and conflicts, Apply (off, and saying why on hover, while anything blocks it), and the `⋯` with Discard. */
export function ReviewHeader({ proposal, itemCount, conflictCount, blockers, disabled, onApply, onDiscard }: ReviewHeaderProps) {
  const journey = proposal.destination !== "deployment" && "journey" in proposal.destination ? proposal.destination.journey : undefined;
  const open = proposal.status === "open";
  return (
    <section className="stack route-header" aria-label={proposal.draft.title} data-testid="review-header">
      <div className="row">
        <h1 data-testid="proposal-title">{proposal.draft.title}</h1>
        <Badge tone={open ? "plain" : proposal.status === "applied" ? "good" : "warn"} data-testid="proposal-status" data-status={proposal.status}>
          {proposal.status}
        </Badge>
        <span className="muted small" data-testid="review-count">
          {itemCount} {itemCount === 1 ? "item" : "items"}
        </span>
        {conflictCount === 0 ? null : (
          <Badge tone="bad" data-testid="review-conflicts">
            {conflictCount} {conflictCount === 1 ? "conflict" : "conflicts"}
          </Badge>
        )}
        {proposal.status === "applied" && journey !== undefined ? (
          <Link to={canvasPath(journey, DEFAULT_VIEW)} data-testid="applied-journey">
            Open the journey
          </Link>
        ) : null}
        <span className="spacer" />
        {open ? (
          <Button primary disabled={disabled || blockers.length > 0} title={blockers.length === 0 ? undefined : blockers.map((blocker) => BLOCKER_WORDS[blocker]).join(" ")} data-testid="apply-proposal" data-blockers={blockers.join(" ")} onClick={onApply}>
            Apply
          </Button>
        ) : null}
        {open ? (
          <Menu label="Proposal actions" testId="proposal-menu" align="end" trigger={<span aria-hidden="true">⋯</span>}>
            {(close) => (
              <button type="button" role="menuitem" className="menu-item" disabled={disabled} data-testid="discard-proposal" onClick={() => { close(); onDiscard(); }}>
                Discard the proposal
              </button>
            )}
          </Menu>
        ) : null}
      </div>
    </section>
  );
}

/** The chips that keep the review to what needs a decision or one kind of change; each says how many. */
export function FilterChips({ filter, counts, onPick }: { filter: ReviewFilter; counts: Record<ReviewFilter, number>; onPick: (filter: ReviewFilter) => void }) {
  return (
    <div className="journey-chips" role="group" aria-label="Show" data-testid="review-filters">
      {REVIEW_FILTERS.filter((each) => each === "all" || counts[each] > 0).map((each) => (
        <button key={each} type="button" className="chip" aria-pressed={filter === each} data-testid={`filter-${each}`} onClick={() => { onPick(each); }}>
          {FILTER_WORDS[each]}
          <span className="chip-count">{counts[each]}</span>
        </button>
      ))}
    </div>
  );
}

const TONES = { good: "good", warn: "warn", bad: "bad", accent: "ring", plain: "plain" } as const;

/** One mark as a word in the tone its kind of change takes. */
function MarkWord({ mark }: { mark: DiffMark }) {
  return <Badge tone={TONES[mark.tone]}>{mark.label}</Badge>;
}

/** The muted foot of a list row: what more there is to say of the change, and what it goes with. */
function foot(entry: ReviewEntry, names: Names): string {
  const parts = [entry.mark.note, entry.cause === undefined ? undefined : `with ${names.node(entry.cause)}`, entry.fields.length === 0 ? undefined : entry.fields.map(fieldName).join(", ")];
  return parts.filter((part) => part !== undefined).join(" · ");
}

/** The List: each marked node with its word and what differs, a cascaded removal under its cause, then the edges. */
export function ReviewList({ entries, diff, names, filter, selected, onPick }: { entries: readonly ReviewEntry[]; diff: GraphDiff; names: Names; filter: ReviewFilter; selected: string | undefined; onPick: (key: string) => void }) {
  const shown = entries.filter((entry) => filter === "all" || filterOf(entry.mark) === filter);
  const edges = filter === "all" || filter === "change";
  if (shown.length === 0 && !(edges && diff.edgesAdded.length + diff.edgesRemoved.length > 0)) {
    return (
      <p className="muted" data-testid="diff-empty">
        As it stands, it changes nothing in the graph.
      </p>
    );
  }
  return (
    <ul className="route-list" data-testid="diff-list">
      {shown.map((entry) => (
        <li
          key={entry.key}
          className="route-list-row"
          data-testid="diff-node"
          data-node={entry.key}
          data-status={filterOf(entry.mark)}
          aria-current={entry.key === selected ? "true" : undefined}
          style={entry.cause === undefined ? undefined : { paddingLeft: "var(--space-6)" }}
        >
          <MarkWord mark={entry.mark} />
          <button type="button" className="link route-list-title" onClick={() => { onPick(entry.key); }}>
            {names.node(entry.key)}
          </button>
          {foot(entry, names) === "" ? null : <span className="muted small">{foot(entry, names)}</span>}
        </li>
      ))}
      {!edges
        ? null
        : [...diff.edgesAdded.map((edge) => ({ edge, status: "added" as const })), ...diff.edgesRemoved.map((edge) => ({ edge, status: "removed" as const }))].map(({ edge, status }) => (
            <li key={`${status}:${edge.node}>${edge.requires}`} className="route-list-row" data-testid="diff-edge" data-status={status}>
              <Badge tone={status === "added" ? "good" : "bad"}>{status === "added" ? DIFF_LABELS.added : DIFF_LABELS.removed}</Badge>
              <span className="route-list-title">
                {names.node(edge.node)} {status === "added" ? "now requires" : "no longer requires"} {names.node(edge.requires)}
              </span>
            </li>
          ))}
    </ul>
  );
}

/** Before and after for the node picked: the fields that differ with what they were and are, or what it is to be added or removed. */
export function BeforeAfter({ node, mark, before, after, names }: { node: string; mark: DiffMark | undefined; before: Graph | undefined; after: Graph | undefined; names: Names }) {
  if (after === undefined && before !== undefined) {
    return <p className="muted small">Not known until the proposal no longer breaks a rule; see the violations.</p>;
  }
  const was = (before?.nodes ?? []).find((each) => each.key === node);
  const now = (after?.nodes ?? []).find((each) => each.key === node);
  if (mark === undefined) {
    return <p className="muted small">Its definition does not change; any pin or answer the proposal sets on it is under its changes.</p>;
  }
  if (was === undefined || now === undefined) {
    return <p className="muted small">{now === undefined ? "It is removed." : "It is new: nothing of it exists yet."}</p>;
  }
  const fields = Object.keys({ ...was, ...now }).filter((field) => field !== "key" && JSON.stringify((was as Record<string, unknown>)[field] ?? null) !== JSON.stringify((now as Record<string, unknown>)[field] ?? null));
  if (fields.length === 0) {
    return <p className="muted small">Nothing in the graph differs; the choice below is about what the journey keeps.</p>;
  }
  return (
    <dl className="stack" data-testid="before-after">
      {fields.map((field) => (
        <div key={field} className="stack" data-testid="changed-field" data-field={field}>
          <dt className="muted small">{fieldName(field)}</dt>
          <dd>
            <s>{fieldValueWords(field, (was as Record<string, unknown>)[field], names)}</s> {"→"} {fieldValueWords(field, (now as Record<string, unknown>)[field], names)}
          </dd>
        </div>
      ))}
    </dl>
  );
}

/** The inspector's item editor for the node picked: its word, what changes, the choices it needs, and its own changes. */
export function ItemEditor({ title, mark, back, children, footer }: { title: string; mark: DiffMark | undefined; back: string; children: ReactNode; footer: ReactNode }) {
  return (
    <aside className="detail-panel panel stack item-editor" aria-label={title} data-testid="item-editor">
      <div className="stack">
        {mark === undefined ? null : (
          <span>
            <MarkWord mark={mark} />
          </span>
        )}
        <h2 data-testid="detail-title">{title}</h2>
      </div>
      {children}
      <Link to={back}>Back to the proposal</Link>
      {footer}
    </aside>
  );
}
