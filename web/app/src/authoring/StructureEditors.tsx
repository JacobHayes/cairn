// The edits to a node's place in the graph, each one patch sent at once and previewed first:
// its requirements (A3: drawn on the canvas or picked, the engine refusing what it cannot take;
// removed), where it sits (A2: moved into another container), who participates
// by role (A6, A7, E2), its breakdown by hand (B10: children added; a journey's placeholder
// marked atomic), and its removal with the cascade shown first (A18).
import { OWNER_KIND } from "@cairn/client";
import { useId, useState } from "react";

import { useDraft } from "../data/drafts.ts";
import { RejectionView } from "../screens/RejectionView.tsx";
import { WriteStatus } from "../screens/WriteStatus.tsx";
import { Button, Input } from "../ui/kit.tsx";
import { CascadeDialog } from "./CascadeDialog.tsx";
import { removalMutations, useRemovalPlan } from "./removal.ts";
import { useConnecting } from "./connect.tsx";
import { countCheck, useValueProblems } from "./form-problems.ts";
import { canHoldChildren, childrenOf, kindOption, moveTargets, nodesByPath, pathOf, rolesForSlot, titleIn, type GraphNode, type Mutation, type NodeKind } from "./graph.ts";
import { mintKey, slugOf, uniqueId } from "./keys.ts";
import { CheckFailed, invalidAttrs, Picker, Problem } from "./parts.tsx";
import { domainOf, isJourney, named, type Authored } from "./target.ts";
import { useAuthorWrite, type AuthorWrite } from "./write.ts";
import { PIECE_KINDS } from "../detail/model.ts";

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
    return <WriteStatus view={write.status} />;
  }
  return <RejectionView rejection={failed.rejection} onRebase={onRetry} onDismiss={write.dismiss} />;
}

/** How many explicit edges touch `key`, in and out (the per-node limit). */
function edgeCount(authored: Authored, key: string): number {
  const out = authored.tree.byKey.get(key)?.requires?.length ?? 0;
  const into = [...authored.tree.byKey.values()].filter((node) => (node.requires ?? []).includes(key)).length;
  return out + into;
}

/** A3: what the node requires, removable, and a requirement added by picking or drawing it. */
export function EdgesEditor({ authored, node }: NodeEditorProps) {
  const write = useAuthorWrite(authored, `edges:${node.key}`, node.title);
  const connecting = useConnecting();
  const [picked, setPicked] = useState("");
  const requires = node.requires ?? [];
  const room = useValueProblems([countCheck("count", "edge_count_per_node_max", "explicit edges on a node", edgeCount(authored, node.key) + 1)]);
  const full = room.problems.count !== undefined;
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
          disabled={write.disabled || picked === "" || full}
          onClick={() => void write.run([{ op: "add_edge", edge: { node: node.key, requires: picked } }]).then((landed) => { if (landed) { setPicked(""); } })}
        >
          Add the requirement
        </Button>
        {connecting === undefined ? null : (
          <Button data-testid="draw-edge" disabled={connecting.view.state === "checking" || connecting.view.state === "saving"} onClick={() => { connecting.start(node.key); }}>
            Draw it on the canvas
          </Button>
        )}
      </span>
      {room.problems.count === undefined ? null : <span className="author-problem">{room.problems.count}</span>}
      <Refusal write={write} />
    </div>
  );
}

/** A2: where the node sits; moving it into another container, or to the top level. */
export function PlaceEditor({ authored, node }: NodeEditorProps) {
  const write = useAuthorWrite(authored, `place:${node.key}`, node.title);
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
          onChange={(event) => void write.run([{ op: "set_node_field", node: node.key, value: { parent: event.target.value === "" ? null : event.target.value } }])}
        />
      </span>
      <Refusal write={write} />
    </div>
  );
}

/** A6, A7, E2: each participation kind wired to a role, inherited, or (a journey's) explicit people. */
export function ParticipationWiring({ authored, node }: NodeEditorProps) {
  const write = useAuthorWrite(authored, `participation:${node.key}`, node.title);
  const roles = authored.graph.roles ?? [];
  const kinds = [{ ...OWNER_KIND, title: "Owner" }, ...(authored.graph.participation_kinds ?? [])];
  return (
    <div className="stack" data-testid="participation-wiring">
      {kinds.map((kind) => {
        const source = node.participations?.[kind.key];
        const offered = rolesForSlot(roles, kind.multi ?? false);
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
export interface Piece {
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

/** B10: the pieces of a breakdown, each a kind (action or deliverable) and a title, edited in place. */
export function PieceRows({ pieces, onChange, problems = {}, message }: { pieces: Piece[]; onChange: (next: Piece[]) => void; problems?: Record<string, string | undefined>; message?: string }) {
  const set = (at: number, piece: Partial<Piece>) => { onChange(pieces.map((each, index) => (index === at ? { ...each, ...piece } : each))); };
  return pieces.map((piece, at) => (
    <span key={at} className="row" data-testid="piece">
      <label className="field">
        <span className="label">Kind</span>
        <Picker aria-label="Kind of piece" value={piece.kind} options={PIECE_KINDS.map(kindOption)} onChange={(event) => { set(at, { kind: event.target.value as NodeKind }); }} />
      </label>
      <label className="field">
        <span className="label">Title</span>
        <Input aria-label="Piece title" placeholder="What it is" {...invalidAttrs(message ?? "", problems[String(at)])} value={piece.title} onChange={(event) => { set(at, { title: event.target.value }); }} />
      </label>
    </span>
  ));
}

/** B10: breaking the node down by hand into children, and a journey's placeholder marked atomic. */
export function Breakdown({ authored, node }: NodeEditorProps) {
  const write = useAuthorWrite(authored, `breakdown:${node.key}`, node.title);
  const [kept, setPieces] = useDraft<Piece[]>(`breakdown:${domainOf(authored)}:${node.key}`);
  const pieces = kept ?? [{ kind: "action", title: "" }];
  const filled = pieces.filter((piece) => piece.title.trim() !== "");
  const checks = useValueProblems(pieces.flatMap((piece, at) => (piece.title.trim() === "" ? [] : [{ field: String(at), kind: "title" as const, what: "A title", value: piece.title.trim() }])));
  const problem = Object.values(checks.problems).find((each) => each !== undefined);
  const message = useId();
  const atomic = authored.graph.state?.nodes?.[node.key]?.atomic === true;
  const children = childrenOf(authored.tree, node.key).length;
  if (!canHoldChildren(node.kind) || node.kind === "group") {
    return null;
  }
  return (
    <div className="stack" data-testid="breakdown">
      <span className="muted small">{children === 0 ? "Nothing beneath it yet." : `${String(children)} beneath it.`}</span>
      <PieceRows pieces={pieces} onChange={setPieces} problems={checks.problems} message={message} />
      <span className="row">
        <Button onClick={() => { setPieces([...pieces, { kind: "action", title: "" }]); }}>Another piece</Button>
        <Button
          primary
          disabled={write.disabled || filled.length === 0 || !checks.checked || problem !== undefined}
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
      {problem === undefined ? null : <Problem id={message}>{problem}</Problem>}
      <CheckFailed failure={checks.failure} retry={checks.retry} />
      <Refusal write={write} />
    </div>
  );
}

/** A18: removing the node, with its cascade shown and previewed before it is sent. */
export function RemoveNode({ authored, node, onRemoved }: NodeEditorProps & { onRemoved: () => void }) {
  const [open, setOpen] = useState(false);
  const plan = useRemovalPlan(authored.graph, authored.tree, open ? { node: node.key } : undefined);
  if (plan === undefined) {
    if (open) {
      return <span className="muted small">Working out what else changes...</span>;
    }
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
      scope={`remove-node:${node.key}`}
      title={`Remove ${node.title}`}
      plan={plan}
      dangling={plan.dangling}
      mutations={removalMutations(plan)}
      onDone={onRemoved}
      onCancel={() => { setOpen(false); }}
    />
  );
}
