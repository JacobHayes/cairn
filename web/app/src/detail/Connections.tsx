// C8, Gating, D4 (design 6.9): what the node waits on, what finishing it would unblock, and why
// it applies or not. Every row is a link that opens that node's detail. Folded, and open when
// the node is blocked, stale, or conditional, which is when there is something to read.
import { useContributions } from "./contributions.ts";
import { ForceInclude } from "./editors.tsx";
import { viaText, namer } from "./explain.ts";
import { foldKey } from "./folds.ts";
import type { NodeDetail, Ready } from "./model.ts";
import { NodeLink, Section } from "./parts.tsx";
import { appliesIf, ownerName, waitingOn } from "./sentence.ts";

/** Gating: why the node is, or is not, in scope, in words; nothing when nothing conditions it. */
function relevanceWords(view: Ready, detail: NodeDetail): React.ReactNode {
  const { relevance } = detail.derived;
  const key = detail.node.key;
  const applies = appliesIf(view, detail);
  if (relevance.forced === true) {
    return "It is included anyway.";
  }
  if (relevance.condition_on == null && detail.node.relevant_when == null) {
    return null;
  }
  const from = relevance.condition_on == null || relevance.condition_on === key ? undefined : relevance.condition_on;
  return (
    <>
      {from === undefined ? "Its own condition: " : <>The condition on <NodeLink view={view} node={from} />: </>}
      {applies === undefined ? "none" : `applies if ${applies}`}.
    </>
  );
}

export function Connections({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const { node, derived } = detail;
  const name = namer(view);
  const waits = waitingOn(view, node.key);
  const frees = useContributions(view, node.key, "leverage", derived.leverage_from);
  const why = relevanceWords(view, detail);
  const count = waits.length + derived.leverage_from.total;
  const open = derived.display_state === "blocked" || derived.display_state === "conditional" || (derived.stale ?? []).length > 0;
  return (
    <Section title="Connections" summary={count === 0 ? undefined : String(count)} open={open} fold={foldKey(node.kind, "connections")} testId="connections">
      {waits.length === 0 ? null : (
        <div className="stack">
          <span className="muted small">Waits on</span>
          <ul className="detail-list" data-testid="blocked-by">
            {waits.map((held) => (
              <li key={`${held.node}:${JSON.stringify(held.via)}`} data-node={held.node}>
                <NodeLink view={view} node={held.node} />{" "}
                <span className="muted small">
                  {ownerName(view, held.node) ?? ""} {held.via === "explicit" || (typeof held.via === "object" && "stage_opening" in held.via) ? "" : viaText(held.via, name)}
                </span>
              </li>
            ))}
          </ul>
        </div>
      )}
      {frees.entries.length === 0 ? null : (
        <div className="stack">
          <span className="muted small">Unblocks when done</span>
          <ul className="detail-list" data-testid="unblocks-when-done">
            {frees.entries.map((entry) => (
              <li key={entry.node} data-node={entry.node}>
                <NodeLink view={view} node={entry.node} />
              </li>
            ))}
          </ul>
        </div>
      )}
      {why === null ? null : (
        <span data-testid="relevance-why" data-status={derived.relevance.value}>
          {why}
        </span>
      )}
      <ForceInclude view={view} detail={detail} />
    </Section>
  );
}
