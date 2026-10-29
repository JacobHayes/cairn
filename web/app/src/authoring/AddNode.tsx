// A1, A2, B4: the palette that adds a node: its kind, its title, and the container it goes in
// (the one drilled into, by default). Its key is minted here and its id made from the title,
// unique among its siblings (Keys and references). A new decision asks its title as a
// yes-or-no question until its form changes the answer type. In a journey it is local (B4).
// The new node opens in its form once it lands.
import { useDraft } from "../data/drafts.ts";
import { wrapRoots } from "../segments/model.ts";
import { Button } from "../ui/kit.tsx";
import { canHoldChildren, childrenOf, newNode, type GraphNode, type NodeKind } from "./graph.ts";
import { NewNodeFields, useNewNodeCheck } from "./NewNodeFields.tsx";
import { Refusal } from "./StructureEditors.tsx";
import { domainOf, type Authored } from "./target.ts";
import { useAuthorWrite } from "./write.ts";

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
  const values = { kind, title, parent };
  const check = useNewNodeCheck(title, authored.tree.byKey.size);
  const unsendable = write.disabled || !check.ready;
  const setTitle = (next: string) => {
    setKept({ ...values, title: next });
  };
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
      <NewNodeFields values={values} onChange={setKept} tree={authored.tree} check={check} />
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
