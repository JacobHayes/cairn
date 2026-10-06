// Date rollover (ARCHITECTURE, Web UI: data flow): a document is derived for the today its
// host computed in the deployment's time zone, so when that date passes, the shell refetches
// what it holds. The browser's Intl knows every zone, so the page can tell when it passes.

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
