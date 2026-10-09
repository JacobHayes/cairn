// B4, A18: Remove in the selection bar while a journey's structure is edited. Each selected node
// (the outermost of any that contain others) is removed with its cascade shown first, one
// confirmation at a time: a removal's rewrites are computed from the graph as it is, so each
// is previewed and sent on the graph the one before left, never merged into one patch.
import { useState } from "react";

import { Button } from "../ui/kit.tsx";
import { CascadeDialog } from "./CascadeDialog.tsx";
import { planRemoval, removalMutations } from "./cascade.ts";
import { ancestorsOf } from "./graph.ts";
import type { Authored } from "./target.ts";

/** The selected nodes no other selected node contains, in the order selected: removing a node takes its subtree. */
export function outermost(authored: Authored, keys: readonly string[]): string[] {
  return keys.filter((key) => authored.tree.byKey.has(key) && !ancestorsOf(authored.tree, key).some((ancestor) => keys.includes(ancestor.key)));
}

export function RemoveSelected({ authored, keys, onRemoved }: { authored: Authored; keys: readonly string[]; onRemoved: (key: string) => void }) {
  const [asking, setAsking] = useState(false);
  const [first] = outermost(authored, keys);
  const plan = asking && first !== undefined ? planRemoval(authored.tree, first) : undefined;
  if (plan === undefined || first === undefined) {
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
      title={`Remove ${authored.tree.byKey.get(first)?.title ?? first}`}
      plan={plan}
      dangling={plan.dangling}
      mutations={removalMutations(plan)}
      onDone={() => { onRemoved(first); }}
      onCancel={() => { setAsking(false); }}
    />
  );
}
