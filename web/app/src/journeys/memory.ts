// Each viewer's last projection is remembered per journey (2.3), for the journey's landing to
// reopen. It is a per-viewer convenience, kept in localStorage, which can be missing or throw
// (a private window, blocked site data, a preview), so every read and write is guarded and the
// screens work without it.
import { DEFAULT_PROJECTION, projectionOf, type Projection } from "./address.ts";

const PREFIX = "cairn:view:";

function read(key: string): string | undefined {
  try {
    return globalThis.localStorage.getItem(`${PREFIX}${key}`) ?? undefined;
  } catch {
    return undefined;
  }
}

function write(key: string, value: string): void {
  try {
    globalThis.localStorage.setItem(`${PREFIX}${key}`, value);
  } catch {
    // Not remembered: the next visit opens the graph.
  }
}

/** The projection last shown for `journey`, or the graph. */
export function recalledProjection(journey: string): Projection {
  return projectionOf(read(journey)) ?? DEFAULT_PROJECTION;
}

/** Remembers that `projection` is where `journey` was last. */
export function rememberProjection(journey: string, projection: Projection): void {
  write(journey, projection);
}
