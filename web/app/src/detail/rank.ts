// Priority, C8: the pieces of "Why this rank" that are sums over the explanation lists: the
// gravity broken down into the node's own weight, what applies downstream, and what only
// counts at the configured undecided discount, split out by the decision it waits on. Pure, so
// the unit tests check the sums without a page.
import type { Schema } from "@cairn/client";

import { unansweredOf, type Ready } from "./model.ts";

type Contribution = Schema<"Contribution">;

/** Nodes downstream and what they add to the gravity. */
export interface Share {
  nodes: number;
  adds: number;
}

/** The gravity's parts: the node's own, what applies, and what waits on each decision. */
export interface GravityParts {
  own: number;
  applies: Share;
  /** Each decision still to be answered with the downstream nodes whose relevance waits on it. */
  conditional: ({ decision: string } & Share)[];
}

/**
 * Splits `gravity` over its contributors. A contributor whose relevance is undecided counts at
 * the discount, under the first open decision it waits on; the node's own weight is what the
 * contributors leave. Only an unfinished list (a capped one) leaves more than that unexplained.
 */
export function gravityParts(view: Ready, gravity: number, entries: readonly Contribution[]): GravityParts {
  const applies: Share = { nodes: 0, adds: 0 };
  const waiting = new Map<string, Share>();
  for (const entry of entries) {
    const relevance = view.derived.nodes[entry.node]?.relevance;
    if (relevance?.value !== "undecided") {
      applies.nodes += 1;
      applies.adds += entry.score;
      continue;
    }
    const decision = unansweredOf(view, entry.node)[0] ?? relevance.decisions?.[0] ?? "";
    const share = waiting.get(decision) ?? { nodes: 0, adds: 0 };
    waiting.set(decision, { nodes: share.nodes + 1, adds: share.adds + entry.score });
  }
  const downstream = applies.adds + [...waiting.values()].reduce((sum, share) => sum + share.adds, 0);
  return {
    own: Math.max(gravity - downstream, 0),
    applies,
    conditional: [...waiting].map(([decision, share]) => ({ decision, ...share })).sort((left, right) => right.adds - left.adds),
  };
}
