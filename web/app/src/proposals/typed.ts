// A node a reviewer has typed in a proposal's additions and not yet added is a draft this tab keeps
// (data/drafts.ts), under the proposal. The review reads it from storage, so it holds Apply
// whichever editor the rail shows, and the additions open on it.
import { useDraftHolding, type Valid } from "../data/drafts.ts";
import type { NewNodeValues } from "../authoring/NewNodeFields.tsx";

/** The draft key of the node being typed for proposal `proposal`. */
export const addNodeKey = (proposal: string) => `proposal-add-node:${proposal}`;

/** Whether a restored draft is what the additions keep. */
export const isNewNodeValues: Valid<NewNodeValues> = (draft): draft is NewNodeValues => typeof draft === "object" && draft !== null && "title" in draft && typeof draft.title === "string";

/** Whether a node is typed in proposal `proposal`'s additions and not yet added. */
export function useNodeTyped(proposal: string): boolean {
  return useDraftHolding<NewNodeValues>(addNodeKey(proposal), (draft) => draft.title.trim() !== "", isNewNodeValues);
}
