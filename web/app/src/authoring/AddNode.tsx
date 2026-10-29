// A1, A2, B4: the palette that adds a node: its kind, its title, and the container it goes in
// (the one drilled into, by default). Its key is minted here and its id made from the title,
// unique among its siblings (Keys and references). A new decision asks its title as a
// yes-or-no question until its form changes the answer type. In a journey it is local (B4).
// The new node opens in its form once it lands.
import { useDraft } from "../data/drafts.ts";
import { Button } from "../ui/kit.tsx";
import { newNode, type GraphNode, type NodeKind } from "./graph.ts";
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
  const add = () => {
    if (unsendable) {
      return;
    }
    const node = newNode(authored.tree, kind, title, parent === "" ? undefined : parent);
    void write.run([{ op: "add_node", node }]).then(added(node));
  };
  return (
    <form className="stack" data-testid="add-node" onSubmit={(event) => { event.preventDefault(); add(); }}>
      <NewNodeFields values={values} onChange={setKept} tree={authored.tree} check={check} />
      <Button type="submit" primary disabled={unsendable}>
        Add
      </Button>
      <Refusal write={write} />
    </form>
  );
}
