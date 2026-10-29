// A1, A2: the fields that make a new node wherever one is added (the palette on a route draft or a
// journey, and a proposal's additions): its kind, its title (read by the engine as the title
// it will be, and held to the graph's node limit), and the container it goes in.
import { useId } from "react";

import { Input } from "../ui/kit.tsx";
import { KINDS } from "../detail/model.ts";
import type { Checked } from "../data/react.ts";
import { countCheck, useValueProblems } from "./form-problems.ts";
import { canHoldChildren, kindOption, nodesByPath, pathOf, type NodeKind, type Tree } from "./graph.ts";
import { CheckFailed, invalidAttrs, Labelled, Picker, Problem } from "./parts.tsx";

/** What is typed for a new node. */
export interface NewNodeValues {
  kind: NodeKind;
  title: string;
  /** The container's key, or empty for the top level. */
  parent: string;
}

/** The engine's reading of a new node's title and of the graph's room for it. */
export interface NewNodeCheck {
  checks: Checked<Record<string, string | undefined>>;
  /** The id of the message the title points to. */
  message: string;
  /** Why the node cannot be added, if it cannot. */
  problem: string | undefined;
  /** A title is typed, the engine has read it and finds nothing wrong. */
  ready: boolean;
}

/** Checks `title` and that a graph of `nodeCount` nodes has room for one more. */
export function useNewNodeCheck(title: string, nodeCount: number): NewNodeCheck {
  const checks = useValueProblems([
    { field: "title", kind: "title", what: "The title", value: title.trim(), optional: true },
    countCheck("count", "node_count_max", "nodes in a graph", nodeCount + 1),
  ]);
  const message = useId();
  const problem = checks.problems.count ?? checks.problems.title;
  return { checks, message, problem, ready: title.trim() !== "" && checks.checked && problem === undefined };
}

/** The kind, the title with its message, and the container, each under its label. */
export function NewNodeFields({ values, onChange, tree, check }: { values: NewNodeValues; onChange: (next: NewNodeValues) => void; tree: Tree; check: NewNodeCheck }) {
  const { checks, message, problem } = check;
  const containers = nodesByPath(tree).filter((node) => canHoldChildren(node.kind));
  return (
    <>
      <Labelled label="Kind"><Picker aria-label="Kind" value={values.kind} options={KINDS.map(kindOption)} onChange={(event) => { onChange({ ...values, kind: event.target.value as NodeKind }); }} /></Labelled>
      <Labelled label="Title"><Input aria-label="New node title" placeholder="Title" {...invalidAttrs(message, problem)} value={values.title} onChange={(event) => { onChange({ ...values, title: event.target.value }); }} /></Labelled>
      {problem === undefined ? null : <Problem id={message}>{problem}</Problem>}
      <CheckFailed failure={checks.failure} retry={checks.retry} />
      <Labelled label="Inside"><Picker aria-label="Inside" value={values.parent} none="At the top level" options={containers.map((node) => ({ value: node.key, label: `${node.title} (${pathOf(tree, node.key)})` }))} onChange={(event) => { onChange({ ...values, parent: event.target.value }); }} /></Labelled>
    </>
  );
}
