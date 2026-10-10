// Priority, C8: why the node ranks where it does (design 6.9, 8.3 to 8.5): the sentence, the four
// terms of the blend as a bar with words, the gravity broken down with what only counts at the
// configured discount split out by the decision it waits on, what finishing it would unblock now
// and what it would not yet free, and the formulas behind it. A container shows one number for its
// whole area. Folded by default, and read on demand for the open node only.
import { rankParts, type NodeRow, type RankPart } from "../acting/why.ts";
import { useContributions } from "./contributions.ts";
import { foldKey, useFold } from "./folds.ts";
import type { NodeDetail, Ready } from "./model.ts";
import { NodeLink, Contributions, Section } from "./parts.tsx";
import { Pieces } from "./Pieces.tsx";
import { gravityParts } from "./rank.ts";
import { useStillWaiting } from "./reads.ts";
import { holdingPhrase, rankClause } from "./sentence.ts";
import { plural, relativeDays } from "./words.ts";

const TERM_NAMES: Record<RankPart["signal"], string> = { urgency: "Urgency", late: "Running late", gravity: "Gravity", unlocks: "Unlocks" };

const percent = (term: number) => `${String(Math.round(term * 100))}%`;

/** One term of the blend in words, with what it adds to the rank. */
function termWords(part: RankPart, detail: NodeDetail): string {
  const { derived } = detail;
  const adds = `+${part.adds.toFixed(2)}`;
  const slack = derived.dates.slack_days;
  switch (part.signal) {
    case "urgency":
      return `${slack == null ? "no deadline" : `latest start ${relativeDays(slack)}`} (${adds})`;
    case "late":
      return `${slack == null ? "past its latest start" : `${plural(-slack, "day")} past its latest start`} (${adds})`;
    case "gravity":
      return `${derived.gravity.toFixed(1)} rides on it, ${percent(part.term)} of the most (${adds})`;
    case "unlocks":
      return `unlocks ${plural(derived.unlocks_from.total, "node")}, ${percent(part.term)} of the most (${adds})`;
  }
}

function Bar({ parts }: { parts: RankPart[] }) {
  return (
    <div className="rank-bar" role="img" aria-label="How much each term adds to the rank">
      {parts.map((part, at) => (
        <span key={part.signal} className={`rank-seg rank-seg-${String((at % 4) + 1)}`} style={{ flexGrow: part.adds }} />
      ))}
    </div>
  );
}

/** The gravity broken down: the node's own weight, what applies downstream, and what waits on each decision at the discount. */
function Gravity({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const { node, derived } = detail;
  const constants = view.inputs.rank;
  const all = useContributions(view, node.key, "gravity", derived.gravity_from);
  const container = detail.children.length > 0 || node.kind === "group";
  const parts = gravityParts(view, derived.gravity, all.entries);
  return (
    <div className="stack" data-testid="gravity-breakdown">
      <span>
        <strong>Gravity</strong> <strong data-testid="gravity">{(derived.subtree_gravity ?? derived.gravity).toFixed(1)}</strong>{" "}
        <span className="muted small">{container ? "how much rides on this whole area" : "how much rides on this"}</span>
      </span>
      {container ? null : (
        <dl className="kv small">
          <dt>Its own weight</dt>
          <dd>{parts.own.toFixed(1)}</dd>
          {parts.applies.nodes === 0 ? null : (
            <>
              <dt>Downstream that applies ({plural(parts.applies.nodes, "node")})</dt>
              <dd>{parts.applies.adds.toFixed(1)}</dd>
            </>
          )}
          {parts.conditional.map((share) => (
            <div key={share.decision} className="kv-row">
              <dt>
                Downstream if {share.decision === "" ? "a decision" : <NodeLink view={view} node={share.decision} />} is answered ({plural(share.nodes, "node")} × {constants.undecided_discount})
              </dt>
              <dd>{share.adds.toFixed(1)}</dd>
            </div>
          ))}
        </dl>
      )}
      {parts.conditional.length === 0 || container ? null : <span className="muted small">× {constants.undecided_discount} is the undecided discount, as configured.</span>}
      {derived.gravity_from.total === 0 ? null : (
        <details>
          <summary className="small">Contributors ({derived.gravity_from.total})</summary>
          <Contributions view={view} entries={all.entries} testId="gravity-from" />
        </details>
      )}
    </div>
  );
}

/** What finishing the node would unblock now, and the dependents it would not yet free. */
function Unlocks({ view, detail, open }: { view: Ready; detail: NodeDetail; open: boolean }) {
  const { node, derived } = detail;
  const frees = useContributions(view, node.key, "unlocks", derived.unlocks_from);
  const waiting = useStillWaiting(view, node.key, open);
  if (derived.unlocks_from.total === 0 && (waiting === undefined || waiting.held.length === 0)) {
    return (
      <span className="muted small" data-testid="unlocks">
        Finishing it unlocks nothing.
      </span>
    );
  }
  return (
    <div className="stack" data-testid="unlocks">
      <span>
        <strong>Unlocks</strong> {derived.unlocks_from.total} <span className="muted small">weighted {derived.unlocks.toFixed(1)}</span>
      </span>
      <Contributions view={view} entries={frees.entries.filter((entry) => entry.other_owner !== true)} testId="unlocks-now" />
      {frees.entries.some((entry) => entry.other_owner === true) ? (
        <div className="stack">
          <span className="muted small">For someone else's work</span>
          <Contributions view={view} entries={frees.entries.filter((entry) => entry.other_owner === true)} testId="unlocks-other" />
        </div>
      ) : null}
      {waiting === undefined || waiting.held.length === 0 ? null : (
        <div className="stack" data-testid="still-waiting">
          <span className="muted small">Still waiting elsewhere</span>
          <ul className="detail-list">
            {waiting.held.map((held) => (
              <li key={held.node} data-node={held.node}>
                <NodeLink view={view} node={held.node} /> <span className="muted small">also waits on </span>
                <Pieces view={view} pieces={held.also_waits_on.flatMap((blocker, at) => [...(at === 0 ? [] : [", "]), ...holdingPhrase(view, { node: blocker.node, via: blocker.via, holder: undefined })])} />
              </li>
            ))}
          </ul>
          {waiting.total > waiting.held.length ? <span className="muted small">and {waiting.total - waiting.held.length} more</span> : null}
        </div>
      )}
    </div>
  );
}

export function WhyRank({ view, detail, position, row }: { view: Ready; detail: NodeDetail; position: number | undefined; row: NodeRow | undefined }) {
  const { node, derived } = detail;
  const gone = derived.display_state === "done" || derived.display_state === "skipped" || derived.display_state === "not_relevant";
  const fold = foldKey(node.kind, "rank");
  const open = useFold(fold, false);
  if (gone) {
    return null;
  }
  if (node.kind === "group") {
    // A group has no rank of its own: what it shows is how much rides on its whole area.
    return (
      <Section title="Gravity" open={false} fold={fold} testId="rank">
        <Gravity view={view} detail={detail} />
      </Section>
    );
  }
  const constants = view.inputs.rank;
  const parts = row?.rank == null ? [] : rankParts(row.rank, constants);
  const sentence = position === undefined ? "Not ranked yet: it can't start until what it waits on is done." : rankClause(view, derived, { position, row }).trim();
  return (
    <Section title="Why this rank" open={false} fold={fold} testId="rank">
      <span data-testid="rank-sentence">{sentence}</span>
      {parts.length === 0 ? null : (
        <>
          <Bar parts={parts} />
          <ul className="detail-list" data-testid="rank-terms">
            {parts.map((part) => (
              <li key={part.signal}>
                <strong>{TERM_NAMES[part.signal]}</strong> {termWords(part, detail)}
              </li>
            ))}
          </ul>
        </>
      )}
      <Gravity view={view} detail={detail} />
      <Unlocks view={view} detail={detail} open={open} />
      <details data-testid="rank-how">
        <summary className="small">How</summary>
        <span className="small">
          Rank is urgency × {constants.urgency}, plus running late × {constants.late}, plus gravity's share × {constants.gravity}, plus unlocks' share × {constants.unlocks}. Urgency looks {constants.horizon_days} days ahead; a
          dependent owned by someone else counts × {constants.other_owner_factor}.
        </span>
      </details>
    </Section>
  );
}
