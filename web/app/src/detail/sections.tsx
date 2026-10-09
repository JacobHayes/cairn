// C8's sections that read the node and its derive: what it is and where it came from, its
// relevance and what produced it, what blocks it and why it is stale, its priority signals
// with their contributors, and its participations. Each takes an optional editor (`edit`),
// the action that section's value invites (B5: force include, weight, participations).
import type { ReactNode } from "react";

import { Badge } from "../ui/kit.tsx";
import { Markdown } from "../ui/markdown.tsx";
import { answerWords, guardFailureText, namer, originText, viaText } from "./explain.ts";
import { INITIAL_STATE, flagsOf, titleOf, type NodeDetail, type Ready } from "./model.ts";
import { statusTone, statusWord } from "../status/words.ts";
import { useContributions } from "./contributions.ts";
import { Contributions, NodeLink, Rationale, Section } from "./parts.tsx";
import { BreakDown } from "../proposals/Entries.tsx";
import { breakable } from "../proposals/model.ts";

/**
 * D8's secondary line: what the state alone leaves out. A node that no longer applies keeps what
 * it recorded; one pending on a decision still to be answered says which and that it counts for
 * nothing until then.
 */
function stateNote(view: Ready, detail: NodeDetail): string | undefined {
  const { derived, record, node } = detail;
  const pending = derived.relevance.pending_on ?? [];
  if (derived.display_state === "conditional" && pending.length > 0) {
    return `Once ${pending.map((key) => titleOf(view, key)).join(", ")} is answered; not counted in priority until then.`;
  }
  if (derived.display_state === "not_relevant" && record.state !== INITIAL_STATE[node.kind]) {
    return `Recorded ${record.state}.`;
  }
  return undefined;
}

/** The node's title, kind, state, path, D3 flags, provenance (from route, local, orphaned), and local edits. */
export function Header({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const { node, record, localEdits } = detail;
  const shown = detail.derived.display_state;
  const note = stateNote(view, detail);
  return (
    <div className="stack" data-testid="detail-header">
      <div className="row">
        <h2 data-testid="detail-title">{node.title}</h2>
        <Badge>{node.kind}</Badge>
        <Badge tone={statusTone(shown)} data-testid="detail-state" data-status={shown}>{statusWord(shown, node.kind)}</Badge>
      </div>
      {note === undefined ? null : <span className="muted small" data-testid="detail-state-note">{note}</span>}
      <span className="muted mono">
        {detail.ancestors.map((ancestor) => (
          <span key={ancestor.key}>
            <NodeLink view={view} node={ancestor.key} /> /{" "}
          </span>
        ))}
        {node.id}
      </span>
      <div className="row" data-testid="flags">
        {flagsOf(detail.derived, record.state).map(({ flag, tone }) => (
          <Badge key={flag} tone={tone} data-testid="flag" data-status={flag}>
            {flag}
          </Badge>
        ))}
      </div>
      <span className="muted small" data-testid="provenance" data-status={record.provenance}>
        {record.provenance === "from_route" ? "From the route" : record.provenance === "orphaned" ? "Orphaned: its route version no longer has it" : "Local to this journey"}
        {localEdits.length === 0 ? "" : `; edited here: ${localEdits.map((edit) => (typeof edit === "string" ? edit : Object.values(edit)[0])).join(", ")}`}
      </span>
      {record.skip_reason == null ? null : <span className="muted small">Skipped: {record.skip_reason}</span>}
    </div>
  );
}

/** The description, and for a decision its prompt, help, and answer. */
export function About({ view, detail, edit }: { view: Ready; detail: NodeDetail; edit?: ReactNode }) {
  const { node, answer, rationale } = detail;
  const decision = node.kind === "decision";
  return (
    <Section title={decision ? "Decision" : "Description"} open={decision || (node.description ?? "") !== ""} testId="about">
      {decision && node.prompt !== undefined ? <Markdown text={node.prompt} data-testid="prompt" /> : null}
      {decision && node.help !== undefined ? <Markdown text={node.help} /> : null}
      {decision ? (
        <span data-testid="answer">
          Answer: {answer === undefined ? <span className="muted small">not answered</span> : <strong>{answerText(view, answer, node)}</strong>}
          {node.fills_role === undefined ? "" : `; fills the role ${roleTitle(view, node.fills_role)}`}
          {node.feeds_milestone === undefined ? null : (
            <>
              ; pins <NodeLink view={view} node={node.feeds_milestone} />
            </>
          )}
        </span>
      ) : null}
      {decision && answer !== undefined ? <Rationale text={rationale} /> : null}
      {node.description == null || node.description === "" ? (
        decision ? null : <span className="muted small">No description.</span>
      ) : (
        <Markdown text={node.description} data-testid="description" />
      )}
      {edit}
    </Section>
  );
}

/** A role by its title, or its key when it has none. */
export function roleTitle(view: Ready, role: string): string {
  return (view.journey.graph.roles ?? []).find((each) => each.key === role)?.title ?? role;
}

/** A participation kind by its title (the built-in owner kind has none in the graph), or its key when it has none. */
export function kindTitle(view: Ready, kind: string): string {
  return kind === "k_owner" ? "Owner" : ((view.journey.graph.participation_kinds ?? []).find((each) => each.key === kind)?.title ?? kind);
}

/** E6: the entity a key names now, following the aliases a merge left behind. */
export function resolveEntity(view: Ready, key: string): string {
  const aliases = view.inputs.deployment.aliases ?? {};
  let found = key;
  const seen = new Set<string>();
  while (aliases[found] !== undefined && !seen.has(found)) {
    seen.add(found);
    found = aliases[found] ?? found;
  }
  return found;
}

/** An entity's name from the deployment (E6), through any merge, or its key. */
export function entityName(view: Ready, key: string): string {
  const resolved = resolveEntity(view, key);
  return (view.inputs.deployment.entities ?? []).find((entity) => entity.key === resolved)?.name ?? key;
}

/** An answer as people read it: choices by their labels, entities by name. */
export function answerText(view: Ready, answer: NodeDetail["answer"] & object, decision?: NodeDetail["node"]): string {
  const label = (id: string) => {
    const choice = (decision?.choices ?? []).find((each) => (typeof each === "string" ? each : each.id) === id);
    return choice === undefined || typeof choice === "string" ? id : choice.title;
  };
  if ("single_choice" in answer) {
    return label(answer.single_choice);
  }
  if ("multi_choice" in answer) {
    return answer.multi_choice.length === 0 ? "none" : answer.multi_choice.map(label).join(", ");
  }
  if ("entity" in answer) {
    return entityName(view, answer.entity);
  }
  if ("entity_list" in answer) {
    return answer.entity_list.length === 0 ? "none" : answer.entity_list.map((key) => entityName(view, key)).join(", ");
  }
  return answerWords(answer);
}

/** Relevance and the ancestor or decision that produced it (C8, Gating). */
export function Relevance({ view, detail, edit }: { view: Ready; detail: NodeDetail; edit?: ReactNode }) {
  const relevance = detail.derived.relevance;
  const key = detail.node.key;
  return (
    <Section title="Relevance" summary={relevance.value.replace("_", " ")} testId="relevance">
      <span data-testid="relevance-why" data-status={relevance.value}>
        {relevance.forced === true ? "Force included. " : null}
        {relevance.condition_on == null && detail.node.relevant_when == null ? (
          "No condition applies."
        ) : relevance.condition_on == null || relevance.condition_on === key ? (
          "By its own condition."
        ) : (
          <>
            By the condition on <NodeLink view={view} node={relevance.condition_on} />.
          </>
        )}
      </span>
      {(relevance.decisions ?? []).length === 0 ? null : (
        <span>
          Reads:{" "}
          {(relevance.decisions ?? []).map((decision) => (
            <span key={decision}>
              <NodeLink view={view} node={decision} />{" "}
            </span>
          ))}
        </span>
      )}
      {edit}
    </Section>
  );
}

/** What blocks it (D1) and through which ancestors; why it is stale (D4); its snooze (B6). */
export function Blocking({ view, detail, edit }: { view: Ready; detail: NodeDetail; edit?: ReactNode }) {
  const name = namer(view);
  const { blocked_by: blockedBy = [], blocked_through: through = [], stale = [], snoozed } = detail.derived;
  const summary = [
    blockedBy.length + through.length === 0 ? "nothing blocks it" : `${String(blockedBy.length + through.length)} blocking`,
    stale.length === 0 ? undefined : "stale",
    snoozed == null ? undefined : "snoozed",
  ].filter((part) => part !== undefined);
  return (
    <Section title="Blocking and flags" summary={summary.join(", ")} open={stale.length > 0 || detail.derived.needs_breakdown === true} testId="blocking">
      {blockedBy.length === 0 ? null : (
        <ul className="detail-list" data-testid="blocked-by">
          {blockedBy.map((blocker) => (
            <li key={`${blocker.node}:${JSON.stringify(blocker.via)}`}>
              <NodeLink view={view} node={blocker.node} /> <span className="muted small">({viaText(blocker.via, name)})</span>
            </li>
          ))}
        </ul>
      )}
      {through.length === 0 ? null : (
        <span data-testid="blocked-through">
          Blocked through{" "}
          {through.map((ancestor) => (
            <span key={ancestor}>
              <NodeLink view={view} node={ancestor} />{" "}
            </span>
          ))}
        </span>
      )}
      {stale.length === 0 ? null : (
        <ul className="detail-list" data-testid="stale">
          {stale.map((failure) => (
            <li key={JSON.stringify(failure)} data-testid="stale-reason">
              Stale: {guardFailureText(failure, name)}
            </li>
          ))}
        </ul>
      )}
      {detail.derived.unassigned === true ? <span>No owner (unassigned).</span> : null}
      {detail.derived.needs_breakdown === true ? <span>A placeholder with nothing beneath it: needs breakdown.</span> : null}
      {breakable(detail.node) ? <BreakDown ready={view} node={detail.node} /> : null}
      {edit}
    </Section>
  );
}

/** Gravity and leverage with the nodes that make them up, leverage split by owner (Priority). */
export function Priority({ view, detail, edit }: { view: Ready; detail: NodeDetail; edit?: ReactNode }) {
  const { leverage, rank, subtree_gravity: area, gravity_from: gravityFrom, leverage_from: leverageFrom } = detail.derived;
  const gravity = area ?? detail.derived.gravity;
  const key = detail.node.key;
  const gravityAll = useContributions(view, key, "gravity", gravityFrom);
  const leverageAll = useContributions(view, key, "leverage", leverageFrom);
  const others = leverageAll.entries.filter((entry) => entry.other_owner === true);
  const same = leverageAll.entries.filter((entry) => entry.other_owner !== true);
  const reading = !gravityAll.complete || !leverageAll.complete;
  return (
    <Section title="Priority" summary={`gravity ${gravity.toFixed(2)}, leverage ${leverage.toFixed(2)}`} testId="priority">
      <span>Rank: {rank == null ? "not ranked" : rank.toFixed(3)}</span>
      {area === undefined ? (
        <span>
          Gravity <strong data-testid="gravity">{gravity.toFixed(2)}</strong>, from {gravityFrom.total} downstream:
        </span>
      ) : (
        <>
          <span>
            Gravity <strong data-testid="gravity">{gravity.toFixed(2)}</strong>: its whole area, the container, its open work, and everything
            downstream of any of it.
          </span>
          <span className="muted small">What follows the container itself ({detail.derived.gravity.toFixed(2)}, which orders it), from {gravityFrom.total}:</span>
        </>
      )}
      <Contributions view={view} entries={gravityAll.entries} testId="gravity-from" />
      <span>
        Leverage <strong>{leverage.toFixed(2)}</strong>: what completing it frees, for the same owner and for others:
      </span>
      <span className="muted small">Same owner</span>
      <Contributions view={view} entries={same} testId="leverage-same" />
      <span className="muted small">Other owners</span>
      <Contributions view={view} entries={others} testId="leverage-other" />
      {reading ? <span className="muted small">Reading the rest of the contributors...</span> : null}
      {edit}
    </Section>
  );
}

/** Each participation kind's entities and where they come from (E2). */
export function Participations({ view, detail, edit }: { view: Ready; detail: NodeDetail; edit?: ReactNode }) {
  const kinds = Object.entries(detail.derived.participations ?? {});
  return (
    <Section title="Participations" summary={kinds.length === 0 ? "none" : `${String(kinds.length)} kinds`} testId="participations">
      {kinds.length === 0 ? <span className="muted small">No one participates.</span> : null}
      <ul className="detail-list">
        {kinds.map(([kind, participation]) => (
          <li key={kind} data-testid="participation" data-kind={kind}>
            <strong>{kindTitle(view, kind)}</strong>:{" "}
            {participation.entities.length === 0 ? "none" : participation.entities.map((key) => entityName(view, key)).join(", ")}{" "}
            <span className="muted small">({originText(participation.origin, view)})</span>
          </li>
        ))}
      </ul>
      {edit}
    </Section>
  );
}
