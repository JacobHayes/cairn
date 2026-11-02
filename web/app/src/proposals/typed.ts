// A node a reviewer has typed in a proposal's additions and not yet added is a draft this tab keeps
// (data/drafts.ts), under the proposal. The review reads it from storage, so it holds Apply
// whichever editor the rail shows, and the additions open on it.
import { useDraftHolding } from "../data/drafts.ts";
import type { NewNodeValues } from "../authoring/NewNodeFields.tsx";

/** The draft key of the node being typed for proposal `proposal`. */
export const addNodeKey = (proposal: string) => `proposal-add-node:${proposal}`;

/** Whether a node is typed in proposal `proposal`'s additions and not yet added. */
export function useNodeTyped(proposal: string): boolean {
  return useDraftHolding<NewNodeValues>(addNodeKey(proposal), (draft) => draft.title.trim() !== "");
}
