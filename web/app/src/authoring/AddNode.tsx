// A1, A2, B4: the palette that adds a node: its kind, its title, and the container it goes in
// (the one drilled into, by default). Its key is minted here and its id made from the title,
// unique among its siblings (Keys and references). A new decision asks its title as a
// yes-or-no question until its form changes the answer type. In a journey it is local (B4).
// The new node opens in its form once it lands.
import { NODE_COUNT_MAX } from "@cairn/client";
import { useId } from "react";
import { useDraft } from "../data/drafts.ts";
import { wrapRoots } from "../segments/model.ts";
import { Button, Input } from "../ui/kit.tsx";
import { canHoldChildren, childrenOf, kindOption, newNode, nodesByPath, pathOf, type GraphNode, type NodeKind } from "./graph.ts";
import { useValueProblems } from "./form-problems.ts";
import { CheckFailed, invalidAttrs, Picker, Problem } from "./parts.tsx";
import { Refusal } from "./StructureEditors.tsx";
import { domainOf, type Authored } from "./target.ts";
import { useAuthorWrite } from "./write.ts";
import { KINDS } from "../detail/model.ts";

export interface AddNodeProps {
  authored: Authored;
  /** The container a new node goes in unless another is picked. */
  container: string | undefined;
  /** Called with the new node once it lands. */
  onAdded: (node: GraphNode) => void;
}

/** C19: why a segment's second top-level node is not added as asked, and what is offered: under the root, or under a group with it when it is a leaf. */
function OneRoot({ root, disabled, onUnder, onWrap }: { root: GraphNode; disabled: boolean; onUnder: () => void; onWrap: () => void }) {
  const holds = canHoldChildren(root.kind);
  return (
    <p className="stack" data-testid="one-root">
      <span>A segment has one top-level node.{holds ? "" : " Put these 2 under a group."}</span>
      <span>
        <Button primary disabled={disabled} onClick={holds ? onUnder : onWrap}>
          {holds ? `Put it under ${root.title}` : "Wrap in a group"}
        </Button>
      </span>
    </p>
  );
}

export function AddNode({ authored, container, onAdded }: AddNodeProps) {
  const write = useAuthorWrite(authored, `add-node:${container ?? "top"}`);
  // The palette's choices are kept across reloads like any unsent form (ARCHITECTURE, Web UI).
  const [kept, setKept] = useDraft<{ kind: NodeKind; title: string; parent: string }>(`add-node:${domainOf(authored)}`);
  const { kind, title, parent } = kept ?? { kind: "deliverable", title: "", parent: container ?? "" };
  const setKind = (next: NodeKind) => {
    setKept({ kind: next, title, parent });
  };
  const setTitle = (next: string) => {
    setKept({ kind, title: next, parent });
  };
  const setParent = (next: string) => {
    setKept({ kind, title, parent: next });
  };
  const containers = nodesByPath(authored.tree).filter((node) => canHoldChildren(node.kind));
  const full = authored.tree.byKey.size >= NODE_COUNT_MAX;
  const checks = useValueProblems([{ field: "title", kind: "title", what: "The title", value: title.trim(), optional: true }]);
  const message = useId();
  const problem = full ? `A graph holds at most ${String(NODE_COUNT_MAX)} nodes.` : checks.problems.title;
  const unsendable = write.disabled || title.trim() === "" || !checks.checked || problem !== undefined;
  const added = (node: GraphNode) => (landed: boolean) => {
    if (landed) {
      setTitle("");
      onAdded(node);
    }
  };
  const roots = childrenOf(authored.tree, undefined);
  // C19: a segment has one top-level node. A second is not sent (the engine would refuse it); the
  // palette offers to put it under the root, or under a group with the root when that is a leaf.
  const root = roots[0];
  const second = authored.route?.header.kind === "segment" && parent === "" && root !== undefined;
  const add = () => {
    if (second || unsendable) {
      return;
    }
    const node = newNode(authored.tree, kind, title, parent === "" ? undefined : parent);
    void write.run([{ op: "add_node", node }]).then(added(node));
  };
  const addUnder = (under: GraphNode) => {
    const node = newNode(authored.tree, kind, title, under.key);
    void write.run([{ op: "add_node", node }]).then(added(node));
  };
  const wrap = (rest: GraphNode) => {
    const node = newNode(authored.tree, kind, title, undefined);
    void write.run(wrapRoots([rest], authored.route?.header.name ?? "", [node])).then(added(node));
  };
  return (
    <form className="stack" data-testid="add-node" onSubmit={(event) => { event.preventDefault(); add(); }}>
      <Picker aria-label="Kind" value={kind} options={KINDS.map(kindOption)} onChange={(event) => { setKind(event.target.value as NodeKind); }} />
      <Input aria-label="New node title" placeholder="Title" {...invalidAttrs(message, problem)} value={title} onChange={(event) => { setTitle(event.target.value); }} />
      {problem === undefined ? null : <Problem id={message}>{problem}</Problem>}
      <CheckFailed failure={checks.failure} retry={checks.retry} />
      <Picker aria-label="Inside" value={parent} none="At the top level" options={containers.map((node) => ({ value: node.key, label: `${node.title} (${pathOf(authored.tree, node.key)})` }))} onChange={(event) => { setParent(event.target.value); }} />
      {second ? (
        <OneRoot root={root} disabled={unsendable} onUnder={() => { addUnder(root); }} onWrap={() => { wrap(root); }} />
      ) : (
        <Button type="submit" primary disabled={unsendable}>
          Add
        </Button>
      )}
      <Refusal write={write} />
    </form>
  );
}
