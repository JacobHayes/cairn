// A1, A2, B4: the palette that adds a node: its kind, its title, and the container it goes in
// (the one drilled into, by default). Its key is minted here and its id made from the title,
// unique among its siblings (Identity and references). A new decision asks its title as a
// yes-or-no question until its form changes the answer type. In a journey it is local (B4).
// The new node opens in its form once it lands.
import { useDraft } from "../data/drafts.ts";
import { wrapRoots } from "../segments/model.ts";
import { Button, Field } from "../ui/kit.tsx";
import { KINDS, canHoldChildren, childrenOf, nodesByPath, pathOf, type GraphNode, type NodeKind } from "./graph.ts";
import { mintKey, slugOf, uniqueId } from "./keys.ts";
import { NODE_COUNT_MAX, TITLE_BYTES_MAX, overBytes } from "./limits.ts";
import { Picker } from "./parts.tsx";
import { Refusal } from "./StructureEditors.tsx";
import { domainOf, type Authored } from "./target.ts";
import { useAuthorWrite } from "./write.ts";

/** A new node of `kind` titled `title` under `parent`, with a fresh key and a sibling-unique id. */
export function newNode(authored: Authored, kind: NodeKind, title: string, parent: string | undefined): GraphNode {
  const taken = new Set(childrenOf(authored.tree, parent).map((child) => child.id));
  const node: GraphNode = { key: mintKey("n_"), id: uniqueId(slugOf(title), taken), kind, title: title.trim() };
  const placed = parent === undefined ? node : { ...node, parent };
  return kind === "decision" ? { ...placed, prompt: title.trim(), answer_type: "boolean" } : placed;
}

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
  const write = useAuthorWrite(authored);
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
  const problem = full ? `A graph holds at most ${String(NODE_COUNT_MAX)} nodes.` : overBytes(title, TITLE_BYTES_MAX, "The title");
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
    if (second) {
      return;
    }
    const node = newNode(authored, kind, title, parent === "" ? undefined : parent);
    void write.run([{ op: "add_node", node }]).then(added(node));
  };
  const addUnder = (under: GraphNode) => {
    const node = newNode(authored, kind, title, under.key);
    void write.run([{ op: "add_node", node }]).then(added(node));
  };
  const wrap = (rest: GraphNode) => {
    const node = newNode(authored, kind, title, undefined);
    void write.run(wrapRoots([rest], authored.route?.header.name ?? "", [node])).then(added(node));
  };
  return (
    <form className="stack" data-testid="add-node" onSubmit={(event) => { event.preventDefault(); add(); }}>
      <Picker aria-label="Kind" value={kind} options={KINDS.map((each) => ({ value: each, label: each }))} onChange={(event) => { setKind(event.target.value as NodeKind); }} />
      <Field aria-label="New node title" placeholder="Title" value={title} onChange={(event) => { setTitle(event.target.value); }} />
      <Picker aria-label="Inside" value={parent} none="At the top level" options={containers.map((node) => ({ value: node.key, label: `${node.title} (${pathOf(authored.tree, node.key)})` }))} onChange={(event) => { setParent(event.target.value); }} />
      {second ? (
        <OneRoot root={root} disabled={write.disabled || title.trim() === "" || problem !== undefined} onUnder={() => { addUnder(root); }} onWrap={() => { wrap(root); }} />
      ) : (
        <Button type="submit" primary disabled={write.disabled || title.trim() === "" || problem !== undefined}>
          Add
        </Button>
      )}
      {problem === undefined ? null : <span className="author-problem">{problem}</span>}
      <Refusal write={write} />
    </form>
  );
}
