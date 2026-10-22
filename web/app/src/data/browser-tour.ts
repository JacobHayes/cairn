// The in-browser host's place in the tour of Cairn: one record in the browser's storage, written
// with the same compare-and-set the server's is, so the tour's lifecycle is the same on both
// hosts. Every journey of this host reseeds on load, so a started record always names a
// journey that is gone after a reload, which the tour reads as a run that is over. Storage that
// refuses (a private window) leaves the record in memory for the page.
import type { Tour, TourWrite, TourWriteRequest } from "./host.ts";
import { todayIn } from "./today.ts";

const KEY = "cairn:tour";

/** What is kept: the record, without today. */
type Kept = Omit<Tour, "today">;

const NONE: Kept = { state: "none" };

function storage(): Storage | undefined {
  try {
    return globalThis.localStorage;
  } catch {
    return undefined;
  }
}

/** Whether `kept` is a record this module wrote: a state, and a practice journey exactly when started. */
function valid(kept: unknown): kept is Kept {
  if (typeof kept !== "object" || kept === null || !("state" in kept)) {
    return false;
  }
  const { state } = kept;
  const journey = "practice_journey" in kept ? kept.practice_journey : undefined;
  return (state === "started" ? typeof journey === "string" : state === "finished" || state === "dismissed") && (state === "started" || journey == null);
}

/** The in-browser host's tour record, kept under `cairn:tour`, and today in the root's zone (UTC: the browser host bundles no zone database). */
export function browserTour(now: () => Date = () => new Date()): { tour: () => Promise<Tour>; putTour: (write: TourWriteRequest) => Promise<TourWrite> } {
  let held: Kept = NONE;
  const read = (): Kept => {
    try {
      const text = storage()?.getItem(KEY);
      if (text == null) {
        return held;
      }
      const parsed: unknown = JSON.parse(text);
      return valid(parsed) ? parsed : NONE;
    } catch {
      return held;
    }
  };
  const view = (kept: Kept): Tour => ({ ...kept, today: todayIn("UTC", now()) });
  return {
    tour: () => Promise.resolve(view(read())),
    putTour: (write) => {
      const current = read();
      const expected = write.expect;
      if (current.state !== expected.state || (current.practice_journey ?? null) !== (expected.practice_journey ?? null)) {
        return Promise.resolve({ ok: false, current: view(current) });
      }
      held = write.state === "started" && write.practice_journey != null ? { state: "started", practice_journey: write.practice_journey } : { state: write.state };
      try {
        storage()?.setItem(KEY, JSON.stringify(held));
      } catch {
        // Kept for the page only.
      }
      return Promise.resolve({ ok: true });
    },
  };
}
