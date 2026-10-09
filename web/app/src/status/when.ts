// Dates in plain words, so no surface prints a bare `Oct 12 · 3d`: "Due in 3 days", "3 days late",
// "Decide by Oct 14" (Approachable by default). A date more than two weeks off is a date: the
// distance is only worth a word while it is near. Pure, so the unit tests check it without a page.
import { dateWords, dayOf } from "../timeline/model.ts";

/** Days a date is called by its distance rather than by its name. */
export const NEAR_DAYS = 14;

/** A count of days in words: "1 day", "3 days". */
export function daysWords(days: number): string {
  return days === 1 ? "1 day" : `${String(days)} days`;
}

/** Whole days from `today` to `date`; negative once it has passed. */
export function daysUntil(date: string, today: string): number {
  return dayOf(date) - dayOf(today);
}

/**
 * When a node is due, in words: late once it has passed, a distance while it is near, a date
 * after that. A decision is decided by a date, not due on it.
 */
export function dueWords(date: string, today: string, decision = false): string {
  const days = daysUntil(date, today);
  if (days < 0) {
    return `${daysWords(-days)} late`;
  }
  if (decision) {
    return days === 0 ? "Decide today" : `Decide by ${dateWords(date, today)}`;
  }
  if (days === 0) {
    return "Due today";
  }
  if (days === 1) {
    return "Due tomorrow";
  }
  return days <= NEAR_DAYS ? `Due in ${daysWords(days)}` : `Due ${dateWords(date, today)}`;
}
