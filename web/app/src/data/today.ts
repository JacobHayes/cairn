// Date rollover (ARCHITECTURE, Web UI: data flow): a document is derived for the today (and now) its
// host computed in the deployment's time zone, so when that date passes, or the next time of
// day that would change the derivation, the shell refetches what it holds. The browser's Intl knows every zone, so the page can tell when it passes.

/** The date (YYYY-MM-DD) it is at `at` in `zone`, an IANA time zone name. */
export function todayIn(zone: string, at: Date): string {
  const parts = new Intl.DateTimeFormat("en-CA", {
    timeZone: zone,
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).formatToParts(at);
  const part = (type: Intl.DateTimeFormatPartTypes) => parts.find((found) => found.type === type)?.value ?? "";
  return `${part("year")}-${part("month")}-${part("day")}`;
}

/**
 * How often the shell asks whether the date has passed. A rollover is seen within this
 * long; the check itself is a date format per held journey.
 */
export const ROLLOVER_CHECK_MS = 60_000;

/** The soonest a derivation is asked for again after the moment it said it held until, so a host whose clock is a little behind the page's is not asked in a tight loop. */
export const HOLDS_RETRY_MS = 1_000;
