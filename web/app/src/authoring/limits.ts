// PRACTICES, Explicit limits: a text typed outside a form (a search box, an email chip, an address)
// is held to the engine's byte limit before it leaves the box. The limits themselves come from the
// engine (`*_MAX` in `@cairn/client`, generated from crates/schema/src/limits.rs); the forms that
// build a document's values ask the engine's own check instead (form-problems.ts).

/** A text's length in bytes, as the limits count it (UTF-8). */
export function byteLength(text: string): number {
  return new TextEncoder().encode(text).length;
}

/** Why `text` is past `max` bytes, or undefined within it. */
export function overBytes(text: string, max: number, what: string): string | undefined {
  const bytes = byteLength(text);
  return bytes > max ? `${what} is ${String(bytes)} bytes; at most ${String(max)}.` : undefined;
}
