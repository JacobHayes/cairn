// What a screen says for a thing it holds a key for but no longer has (a removed node, a
// role the journey dropped): never the key, which is an internal id and means nothing to a
// reader (e2e/internal-ids.ts fails a screen that shows one).

/** A node the journey no longer holds. */
export const NODE_GONE = "an item that was removed";
/** A role the graph no longer holds. */
export const ROLE_GONE = "a role that was removed";
/** A participation kind the graph no longer holds. */
export const KIND_GONE = "a kind that was removed";
/** An entity the deployment does not hold. */
export const ENTITY_GONE = "someone unknown";
