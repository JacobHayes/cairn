// PRACTICES, Explicit limits: the limits an editor enforces before it sends (each editor
// enforces its limits before sending), mirrored from crates/schema/src/limits.rs, which the
// engine rejects a patch past. A value past one never leaves the form; the engine stays the
// authority, so a limit changed in Rust alone still holds, only less politely.

/** Nodes per graph. */
export const NODE_COUNT_MAX = 2_000;
/** Explicit edges per node, in plus out. */
export const EDGE_COUNT_PER_NODE_MAX = 64;
/** Roles per graph. */
export const ROLE_COUNT_MAX = 32;
/** Participation kinds per graph. */
export const KIND_COUNT_MAX = 32;
/** Choices per decision. */
export const CHOICE_COUNT_PER_DECISION_MAX = 32;
/** Condition tree depth. */
export const CONDITION_DEPTH_MAX = 8;
/** Condition clauses, leaves and combinators alike. */
export const CONDITION_CLAUSE_COUNT_MAX = 16;
/** An id slug, in bytes. */
export const ID_BYTES_MAX = 64;
/** A title, in bytes. */
export const TITLE_BYTES_MAX = 256;
/** A description, note, prompt, help text, or resource body, in bytes. */
export const BODY_BYTES_MAX = 64 * 1024;
/** A link (a URL), in bytes. */
export const LINK_BYTES_MAX = 4 * 1024;
/** Resources per node. */
export const RESOURCE_COUNT_PER_NODE_MAX = 16;
/** A date offset or an estimate, in days. */
export const OFFSET_DAYS_MAX = 365;
/** A weight. */
export const WEIGHT_MAX = 1_000;

/** A text's length in bytes, as the limits count it (UTF-8). */
export function byteLength(text: string): number {
  return new TextEncoder().encode(text).length;
}

/** Why `text` is past `max` bytes, or undefined within it. */
export function overBytes(text: string, max: number, what: string): string | undefined {
  const bytes = byteLength(text);
  return bytes > max ? `${what} is ${String(bytes)} bytes; at most ${String(max)}.` : undefined;
}

/** Why `value` is not a whole number from 0 to `max`, or undefined when it is one. */
export function outOfRange(value: number, max: number, what: string): string | undefined {
  return Number.isInteger(value) && value >= 0 && value <= max ? undefined : `${what} is a whole number from 0 to ${String(max)}.`;
}

/** Why `text` is not a URL as the schema takes one (scheme:rest, no whitespace), or undefined. */
export function urlProblem(text: string): string | undefined {
  return /^[A-Za-z][A-Za-z0-9+.-]*:\S+$/.test(text) ? overBytes(text, LINK_BYTES_MAX, "The address") : "An address is a URL: a scheme, a colon, and no spaces (https://...).";
}
