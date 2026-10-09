// What the browser tests of the decision view, the timeline, and the Summary page share: the
// fixtures' journeys, and the day the browser host reads them on so what is overdue or due
// soon does not move with the calendar.

/** The day the browser host's tests read the fixtures on: the scenario matrix's clock. */
export const FIXED_TODAY = "2026-10-06";

/** Each fixture's journey (fixtures/README.md), by the fixture's directory. */
export const FIXTURE_JOURNEYS = {
  "vendor-evaluation": "j_vendor_eval",
  "hiring-loop": "j_hiring",
  "product-launch": "j_launch",
  "bake-off": "j_bakeoff",
} as const;
