// B4, A18: Remove in the inspector's selection panel while a journey's structure is edited. Each selected node
// (the outermost of any that contain others) is removed with its cascade shown first, one
// confirmation at a time: a removal's rewrites are computed from the graph as it is, so each
// is previewed and sent on the graph the one before left, never merged into one patch.
import { useState } from "react";

import { Button } from "../ui/kit.tsx";
import { CascadeDialog } from "./CascadeDialog.tsx";
import { removalMutations, useRemovalPlan } from "./removal.ts";
import { ancestorsOf, titleIn } from "./graph.ts";
import type { Authored } from "./target.ts";

/** The selected nodes no other selected node contains, in the order selected: removing a node takes its subtree. */
export function outermost(authored: Authored, keys: readonly string[]): string[] {
  return keys.filter((key) => authored.tree.byKey.has(key) && !ancestorsOf(authored.tree, key).some((ancestor) => keys.includes(ancestor.key)));
}

export function RemoveSelected({ authored, keys }: { authored: Authored; keys: readonly string[] }) {
  const [asking, setAsking] = useState(false);
  const [first] = outermost(authored, keys);
  const plan = useRemovalPlan(authored.graph, authored.tree, asking && first !== undefined ? { node: first } : undefined);
  if (plan === undefined || first === undefined) {
    if (asking && first !== undefined) {
      return <span className="muted small">Working out what else changes...</span>;
    }
    return (
      <Button disabled={first === undefined} data-testid="remove-selected" onClick={() => { setAsking(true); }}>
        Remove {keys.length === 1 ? "it" : `${String(keys.length)} nodes`}...
      </Button>
    );
  }
  return (
    <CascadeDialog
      key={first}
      authored={authored}
      scope="remove-selected"
      title={`Remove ${titleIn(authored.tree, first)}`}
      plan={plan}
      dangling={plan.dangling}
      mutations={removalMutations(plan)}
      // The node leaves the graph and the selection with it; the next one's dialog opens in its place.
      onDone={() => undefined}
      onCancel={() => { setAsking(false); }}
    />
  );
}
