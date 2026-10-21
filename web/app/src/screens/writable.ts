// C14, B11, A19: the one gate on every live write a page offers. A proposal's layer, in any
// mode, makes the page a picture of the proposal until it is closed, and an archived journey
// accepts nothing but its status moves and deletion (the engine's table, generated). Every
// surface that offers a write reads this, and a node's writes do not send without it.
import { STATUS_ACCEPTS_CHANGES } from "@cairn/client";

import type { Ready } from "../detail/model.ts";
import { useLayer } from "../proposals/layer.ts";

/**
 * Whether the page offers live writes: none while a proposal's layer is on, and, when `journey`
 * is named, none to a journey whose status accepts no changes. The journey's status moves and
 * its deletion ask without naming it.
 */
export function useWritable(journey?: Ready): boolean {
  const layer = useLayer();
  return layer === undefined && (journey === undefined || STATUS_ACCEPTS_CHANGES[journey.journey.header.status]);
}
