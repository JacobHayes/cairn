// PRACTICES, Explicit limits: each editor enforces its limits before it sends. The limits
// themselves come from the engine (`*_MAX` in `@cairn/client`, generated from
// crates/schema/src/limits.rs); this file holds the checks that apply them. A value past one
// never leaves the form; the engine stays the authority.
import { LINK_BYTES_MAX } from "@cairn/client";

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
