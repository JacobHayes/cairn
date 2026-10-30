// A18: what removing a node, a role or a participation kind takes with it, and what else must
// change in the same patch. The engine works it out (the removal names everything it removes as
// the author sees it now, and every reference that would dangle comes with the mutation that
// rewrites or removes it, sent first), asked through the derive worker; this reads the answer
// and says it in words.
import type { Schema } from "@cairn/client";

import { DERIVE, type ReadKey, type ReadSource } from "../data/keyed-read.ts";
import { useKeyedRead, useSession } from "../data/react.ts";
import type { Deriver } from "../data/host.ts";
import { titleIn, type Graph, type GraphNode, type Mutation, type Tree } from "./graph.ts";

export type Removal = Schema<"Removal">;
export type RemovalTarget = Schema<"RemovalTarget">;

/** A reference outside what is removed that would dangle, and the mutation that rewrites or removes it. */
export interface Dangling {
  /** The node holding the reference; none for the graph's default owner. */
  node: string | undefined;
  what: Schema<"DanglingKind">;
  /** What it does about it, in words. */
  fix: string;
  mutation: Mutation;
}

/** A removal and its cascade: what to show the author and what to send, in order. */
export interface RemovalPlan {
  /** A node's removal; none for a role or a kind. */
  removal: Removal | undefined;
  /** The nodes removed, the removed one first. */
  nodes: GraphNode[];
  dangling: Dangling[];
}

/** What a fix does, in words. */
function fixWords(each: Schema<"Dangling">, tree: Tree): string {
  const { mutation } = each;
  switch (mutation.op) {
    case "set_node_field": {
      const value = mutation.value as Record<string, unknown>;
      const [name] = Object.keys(value);
      const cleared = Object.values(value)[0] == null;
      switch (name) {
        case "relevant_when":
          return cleared ? "clear its condition" : "drop the clauses that name it from its condition";
        case "due_by":
        case "not_before":
          return cleared ? `clear its ${name} rule` : `measure its ${name} rule without what is removed`;
        case "feeds_milestone":
          return "stop it pinning the milestone";
        default:
          return "stop it filling the role";
      }
    }
    case "clear_participation":
      return "clear its participation";
    case "set_default_owner":
      return "leave the graph with no default owner";
    case "unsnooze":
      return "unsnooze it";
    case "remove_resource":
      return "remove the message draft, which names nothing else";
    case "edit_resource":
      return `drop the placeholders that name it from "${mutation.resource.title ?? "a message draft"}"`;
    default:
      return `rewrite ${each.node == null ? "the graph" : titleIn(tree, each.node ?? "")}`;
  }
}

/** The engine's plan as the dialogs show it. */
function planOf(plan: Schema<"RemovalPlan">, tree: Tree): RemovalPlan {
  return {
    removal: plan.removal ?? undefined,
    nodes: (plan.nodes ?? []).flatMap((key) => tree.byKey.get(key) ?? []),
    dangling: (plan.dangling ?? []).map((each) => ({ node: each.node ?? undefined, what: each.what, fix: fixWords(each, tree), mutation: each.mutation })),
  };
}

/** What a removal plan sends, in order: the rewrites first, on nodes that stay, then the removal. */
export function removalMutations(plan: RemovalPlan): Mutation[] {
  return [...plan.dangling.map((each) => each.mutation), ...(plan.removal === undefined ? [] : [{ op: "remove_node" as const, removal: plan.removal }])];
}

interface Args {
  deriver: Deriver;
  graph: Graph;
  tree: Tree;
  target: RemovalTarget;
}

const PLAN: ReadSource<Args, RemovalPlan | undefined> = {
  policy: DERIVE,
  read: async ({ deriver, graph, tree, target }) => {
    const plan = await deriver.removalPlan(graph, target);
    return plan === undefined ? undefined : planOf(plan, tree);
  },
};

/**
 * A18: the engine's plan for removing `target` from `graph` (a journey's carries its state), once it has answered.
 * `undefined` while it works, or when `target` is none or not in the graph.
 */
export function useRemovalPlan(graph: Graph, tree: Tree, target: RemovalTarget | undefined): RemovalPlan | undefined {
  const { deriver } = useSession();
  const key: ReadKey<Args> | undefined = target === undefined ? undefined : { of: ["removal", JSON.stringify(target)], freshness: [JSON.stringify(graph)], args: { deriver, graph, tree, target } };
  const { view } = useKeyedRead(PLAN, key);
  return view.status === "current" ? view.value : undefined;
}
