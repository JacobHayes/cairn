// The edits to a node's place in the graph, each one patch sent at once and previewed first:
// its requirements (A3: drawn on the canvas or picked, an ancestor or descendant refused with
// the reason; removed), where it sits (A2: moved into another container), who participates
// by role (A6, A7, E2), its breakdown by hand (B10: children added; a journey's placeholder
// marked atomic), and its removal with the cascade shown first (A18).
import { useState } from "react";

import { useDraft } from "../data/drafts.ts";
import { Button, Field } from "../ui/kit.tsx";
import { CascadeDialog } from "./CascadeDialog.tsx";
import { planRemoval, removalMutations } from "./cascade.ts";
import { useConnecting } from "./connect.tsx";
import { EDGE_COUNT_PER_NODE_MAX, TITLE_BYTES_MAX, overBytes } from "./limits.ts";
import { canHoldChildren, childrenOf, edgeRefusal, moveTargets, nodesByPath, pathOf, titleIn, type GraphNode, type Mutation, type NodeKind } from "./graph.ts";
import { mintKey, slugOf, uniqueId } from "./keys.ts";
import { Picker } from "./parts.tsx";
import { domainOf, isJourney, named, type Authored } from "./target.ts";
import { useAuthorWrite, type AuthorWrite } from "./write.ts";

export interface NodeEditorProps {
  authored: Authored;
  node: GraphNode;
}

/**
 * A write's rejection, in a line each. A stale one (H5: someone changed the same thing since
 * the edit was opened) offers to keep the edit on the current version when the form can.
 */
export function Refusal({ write, onRetry }: { write: AuthorWrite; onRetry?: (() => void) | undefined }) {
  const failed = write.failed;
  if (failed === undefined) {
    return null;
  }
  const lines = failed.rejection.rejection === "invalid" ? failed.rejection.violations.map((violation) => violation.message) : ["The graph changed since; try again."];
  return (
    <span className="stack callout callout-bad" role="alert" data-testid="refused">
      {lines.map((line, at) => (
        <span key={at}>{line}</span>
      ))}
      <span className="row">
        {failed.rejection.rejection === "stale" && onRetry !== undefined ? <Button onClick={onRetry}>Keep my edit on the current version</Button> : null}
        <Button onClick={write.dismiss}>Dismiss</Button>
      </span>
    </span>
  );
}

/** How many explicit edges touch `key`, in and out (the per-node limit). */
function edgeCount(authored: Authored, key: string): number {
  const out = authored.tree.byKey.get(key)?.requires?.length ?? 0;
  const into = [...authored.tree.byKey.values()].filter((node) => (node.requires ?? []).includes(key)).length;
  return out + into;
}

/** A3: what the node requires, removable, and a requirement added by picking or drawing it. */
export function EdgesEditor({ authored, node }: NodeEditorProps) {
  const write = useAuthorWrite(authored);
  const connecting = useConnecting();
  const [picked, setPicked] = useState("");
  const requires = node.requires ?? [];
  const refusal = picked === "" ? undefined : edgeRefusal(authored.tree, node.key, picked);
  const full = edgeCount(authored, node.key) >= EDGE_COUNT_PER_NODE_MAX;
  const candidates = nodesByPath(authored.tree).filter((each) => each.key !== node.key && !requires.includes(each.key));
  return (
    <div className="stack" data-testid="edges-editor">
      {requires.length === 0 ? <span className="muted small">It requires nothing explicitly.</span> : null}
      <ul className="detail-list">
        {requires.map((key) => (
          <li key={key} className="row" data-testid="requirement" data-node={key}>
            <span>Requires {titleIn(authored.tree, key)}</span>
            <Button aria-label={`Stop requiring ${titleIn(authored.tree, key)}`} disabled={write.disabled} onClick={() => void write.run([{ op: "remove_edge", edge: { node: node.key, requires: key } }])}>
              Remove
            </Button>
          </li>
        ))}
      </ul>
      <span className="row">
        <Picker aria-label="Require" value={picked} none="Require..." options={candidates.map((each) => ({ value: each.key, label: `${each.title} (${pathOf(authored.tree, each.key)})` }))} onChange={(event) => { setPicked(event.target.value); }} />
        <Button
          disabled={write.disabled || picked === "" || refusal !== undefined || full}
          onClick={() => void write.run([{ op: "add_edge", edge: { node: node.key, requires: picked } }]).then((landed) => { if (landed) { setPicked(""); } })}
        >
          Add the requirement
        </Button>
        {connecting === undefined ? null : <Button onClick={() => { connecting.start(node.key); }}>Draw it on the canvas</Button>}
      </span>
      {refusal === undefined ? null : <span className="author-problem" role="alert" data-testid="edge-refused">{refusal}</span>}
      {full ? <span className="author-problem">At most {EDGE_COUNT_PER_NODE_MAX} explicit edges touch one node.</span> : null}
      <Refusal write={write} />
    </div>
  );
}

/** A2: where the node sits; moving it into another container, or to the top level. */
export function PlaceEditor({ authored, node }: NodeEditorProps) {
  const write = useAuthorWrite(authored);
  const targets = moveTargets(authored.tree, node.key);
  return (
    <div className="stack" data-testid="place-editor">
      <span className="row">
        <span>Inside</span>
        <Picker
          aria-label="Move into"
          value={node.parent ?? ""}
          none="The top level"
          options={targets.map((each) => ({ value: each.key, label: `${each.title} (${pathOf(authored.tree, each.key)})` }))}
          disabled={write.disabled}
          onChange={(event) => void write.run([{ op: "set_node_field", node: node.key, value: { parent: event.target.value === "" ? null : event.target.value } }], ["parent"])}
        />
      </span>
      <Refusal write={write} />
    </div>
  );
}

/** A6, A7, E2: each participation kind wired to a role, inherited, or (a journey's) explicit people. */
export function ParticipationWiring({ authored, node }: NodeEditorProps) {
  const write = useAuthorWrite(authored);
  const roles = authored.graph.roles ?? [];
  const kinds = [{ key: "k_owner", id: "owner", title: "Owner", multi: false }, ...(authored.graph.participation_kinds ?? [])];
  return (
    <div className="stack" data-testid="participation-wiring">
      {kinds.map((kind) => {
        const source = node.participations?.[kind.key];
        const offered = roles.filter((role) => (kind.multi ?? false) || role.multi !== true);
        const value = source === undefined ? "" : Array.isArray(source) ? "explicit" : source;
        const set = (next: string) => {
          const mutation: Mutation = next === "" ? { op: "clear_participation", node: node.key, kind: kind.key } : { op: "set_participation", node: node.key, kind: kind.key, source: next };
          void write.run([mutation]);
        };
        return (
          <span key={kind.key} className="row" data-testid="wiring" data-kind={kind.key}>
            <span>{named(kind)}</span>
            <Picker
              aria-label={`${named(kind)} from`}
              value={value}
              none="Inherited"
              disabled={write.disabled || value === "explicit"}
              options={[...offered.map((role) => ({ value: role.key, label: `the role ${named(role)}` })), ...(value === "explicit" ? [{ value: "explicit", label: "named people (set in Participations)" }] : [])]}
              onChange={(event) => { set(event.target.value); }}
            />
          </span>
        );
      })}
      <Refusal write={write} />
    </div>
  );
}

/** One child a breakdown adds: a kind and a title. */
interface Piece {
  kind: NodeKind;
  title: string;
}

/** B10: the children a breakdown adds, each a new local node under `parent`, ids unique among their siblings. */
export function breakdownMutations(authored: Authored, parent: string, pieces: readonly Piece[]): Mutation[] {
  const taken = new Set(childrenOf(authored.tree, parent).map((child) => child.id));
  return pieces.map((piece): Mutation => {
    const id = uniqueId(slugOf(piece.title), taken);
    taken.add(id);
    return { op: "add_node", node: { key: mintKey("n_"), id, kind: piece.kind, title: piece.title.trim(), parent } };
  });
}

/** B10: breaking the node down by hand into children, and a journey's placeholder marked atomic. */
export function Breakdown({ authored, node }: NodeEditorProps) {
  const write = useAuthorWrite(authored);
  const [kept, setPieces] = useDraft<Piece[]>(`breakdown:${domainOf(authored)}:${node.key}`);
  const pieces = kept ?? [{ kind: "action", title: "" }];
  const filled = pieces.filter((piece) => piece.title.trim() !== "");
  const problem = filled.map((piece) => overBytes(piece.title, TITLE_BYTES_MAX, "A title")).find((each) => each !== undefined);
  const atomic = authored.graph.state?.nodes?.[node.key]?.atomic === true;
  const children = childrenOf(authored.tree, node.key).length;
  if (!canHoldChildren(node.kind) || node.kind === "group") {
    return null;
  }
  return (
    <div className="stack" data-testid="breakdown">
      <span className="muted small">{children === 0 ? "Nothing beneath it yet." : `${String(children)} beneath it.`}</span>
      {pieces.map((piece, at) => (
        <span key={at} className="row" data-testid="piece">
          <Picker aria-label="Kind of piece" value={piece.kind} options={[{ value: "action", label: "action" }, { value: "deliverable", label: "deliverable" }]} onChange={(event) => { setPieces(pieces.map((each, index) => (index === at ? { ...each, kind: event.target.value as NodeKind } : each))); }} />
          <Field aria-label="Piece title" placeholder="What it is" value={piece.title} onChange={(event) => { setPieces(pieces.map((each, index) => (index === at ? { ...each, title: event.target.value } : each))); }} />
        </span>
      ))}
      <span className="row">
        <Button onClick={() => { setPieces([...pieces, { kind: "action", title: "" }]); }}>Another piece</Button>
        <Button
          primary
          disabled={write.disabled || filled.length === 0 || problem !== undefined}
          onClick={() => void write.run(breakdownMutations(authored, node.key, filled)).then((landed) => { if (landed) { setPieces(undefined); } })}
        >
          Break it down
        </Button>
        {isJourney(authored) && node.placeholder === true ? (
          <Button disabled={write.disabled} onClick={() => void write.run([{ op: "set_atomic", node: node.key, atomic: !atomic }])}>
            {atomic ? "Needs breaking down after all" : "Mark atomic: nothing to break down"}
          </Button>
        ) : null}
      </span>
      {problem === undefined ? null : <span className="author-problem">{problem}</span>}
      <Refusal write={write} />
    </div>
  );
}

/** A18: removing the node, with its cascade shown and previewed before it is sent. */
export function RemoveNode({ authored, node, onRemoved }: NodeEditorProps & { onRemoved: () => void }) {
  const [open, setOpen] = useState(false);
  const plan = open ? planRemoval(authored.tree, node.key) : undefined;
  if (plan === undefined) {
    return (
      <span className="row">
        <Button onClick={() => { setOpen(true); }} data-testid="remove-node">
          Remove {node.title}
        </Button>
      </span>
    );
  }
  return (
    <CascadeDialog
      authored={authored}
      title={`Remove ${node.title}`}
      plan={plan}
      dangling={plan.dangling}
      mutations={removalMutations(plan)}
      onDone={onRemoved}
      onCancel={() => { setOpen(false); }}
    />
  );
}
