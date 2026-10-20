// What a screen says for a thing it holds a key for but no longer has (a removed node, a
// role the journey dropped): never the key, which is an internal id and means nothing to a
// reader (e2e/internal-ids.ts fails a screen that shows one).

/** A node the journey no longer holds. */
export const NODE_GONE = "an item that was removed";
/** A node's title, or words for a node the journey no longer holds (`title` is the lookup's result). */
export function nodeTitle(title: string | undefined): string {
  return title ?? NODE_GONE;
}
/** A role the graph no longer holds. */
export const ROLE_GONE = "a role that was removed";
/** A role or participation kind by its title, its id when it has no title, or `gone` for one the graph no longer holds. */
export function slotName(slot: { id: string; title?: string | null | undefined } | undefined, gone: string): string {
  return slot === undefined ? gone : (slot.title ?? slot.id);
}
/** A participation kind the graph no longer holds. */
export const KIND_GONE = "a kind that was removed";
/** An identity the deployment does not hold. */
export const IDENTITY_GONE = "someone unknown";
