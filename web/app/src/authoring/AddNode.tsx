// A1, A2, B4: the palette that adds a node: its kind, its title, and the container it goes in
// (the one drilled into, by default). Its key is minted here and its id made from the title,
// unique among its siblings (Identity and references). A new decision asks its title as a
// yes-or-no question until its form changes the answer type. In a journey it is local (B4).
// The new node opens in its form once it lands.
import { useDraft } from "../data/drafts.ts";
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
  /** Called with the new node's key once it lands. */
  onAdded: (key: string) => void;
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
  const add = () => {
    const node = newNode(authored, kind, title, parent === "" ? undefined : parent);
    void write.run([{ op: "add_node", node }]).then((landed) => {
      if (landed) {
        setTitle("");
        onAdded(node.key);
      }
    });
  };
  return (
    <form className="row" data-testid="add-node" onSubmit={(event) => { event.preventDefault(); add(); }}>
      <Picker aria-label="Kind" value={kind} options={KINDS.map((each) => ({ value: each, label: each }))} onChange={(event) => { setKind(event.target.value as NodeKind); }} />
      <Field aria-label="New node title" placeholder="Title" value={title} onChange={(event) => { setTitle(event.target.value); }} />
      <Picker aria-label="Inside" value={parent} none="At the top level" options={containers.map((node) => ({ value: node.key, label: `${node.title} (${pathOf(authored.tree, node.key)})` }))} onChange={(event) => { setParent(event.target.value); }} />
      <Button type="submit" primary disabled={write.disabled || title.trim() === "" || problem !== undefined}>
        Add
      </Button>
      {problem === undefined ? null : <span className="author-problem">{problem}</span>}
      <Refusal write={write} />
    </form>
  );
}
