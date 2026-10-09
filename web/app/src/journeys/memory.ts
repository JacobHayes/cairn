// Each viewer's last projection per page is remembered per journey (2.3), and the journey's
// last page beside it, for the rail's Recent list to reopen it (2.1). It is a per-viewer convenience, kept in
// localStorage, which can be missing or throw (a private window, blocked site data, a
// preview), so every read and write is guarded and the screens work without it.
import { DEFAULT_PROJECTION, projectionOf, type JourneyPage, type Projection } from "./address.ts";

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
    // Not remembered: the next visit opens the page's default.
  }
}

/** The projection last shown on `page` of `journey`, or the page's default. */
export function recalledProjection(journey: string, page: JourneyPage): Projection {
  return projectionOf(page, read(`${journey}:${page}`)) ?? DEFAULT_PROJECTION[page];
}

/** Remembers that `projection` is where `page` of `journey` was last, and that `page` is the journey's last page. */
export function rememberProjection(journey: string, page: JourneyPage, projection: Projection): void {
  write(`${journey}:${page}`, projection);
  write(`${journey}:page`, page);
}
