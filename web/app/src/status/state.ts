// D8, Gating: the two questions every screen asks of a node's engine values, written once.
// Both read what the engine derived (relevance and display state) and nothing stored.
import type { Schema } from "@cairn/client";

type DisplayState = Schema<"DisplayState">;
type NodeDerived = Schema<"NodeDerived">;

/**
 * D8: nothing is left to do on the node: its display state is done (finished, decided, reached
 * or auto-reached) or skipped (stored or through a container).
 */
export function isFinished(state: DisplayState): boolean {
  return state === "done" || state === "skipped";
}

/**
 * Gating, D8: the node is still to do and in scope: relevant or undecided, and not finished or
 * skipped. A node that waits on an unanswerable decision displays as conditional but is out of
 * scope (not relevant until it is answered), so display state alone cannot say "in scope".
 */
export function isOpen(derived: NodeDerived | undefined): boolean {
  return derived !== undefined && derived.relevance.value !== "not_relevant" && !isFinished(derived.display_state);
}
