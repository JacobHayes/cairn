// D8, Gating: whether a node is finished, read from the display state the engine derived. Whether it is
// still open (to do and in scope) is the engine's `open`, read from the node's derived values.
import type { Schema } from "@cairn/client";

type DisplayState = Schema<"DisplayState">;

/**
 * D8: nothing is left to do on the node: its display state is done (finished, decided
 * or done by its date) or skipped (stored or through a container).
 */
export function isFinished(state: DisplayState): boolean {
  return state === "done" || state === "skipped";
}
