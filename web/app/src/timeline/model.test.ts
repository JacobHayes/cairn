// C13, F7: the timeline's axis: the `final` milestone as the end anchor (and none
// without one), the span holding today and every date, ticks few enough at the finest step
// that fits, and each date placed along the axis.
import { describe, expect, it } from "vitest";

import {
  AXIS_MARGIN_DAYS,
  TICK_COUNT_MAX,
  dateAway,
  dateOf,
  dateWords,
  dayOf,
  endAnchor,
  positionOf,
  rangeWindow,
  timelineAxis,
  type Timeline,
  type TimelineEntry,
} from "./model.ts";

const today = "2026-10-06";

function entry(node: string, date: string, origin: TimelineEntry["origin"], extra: Partial<TimelineEntry> = {}): TimelineEntry {
  return { node, kind: node === "n_meeting" ? "milestone" : "deliverable", date, origin, ...extra };
}

/** A final milestone pinned on 2026-11-20, and a deliverable pinned after it. */
const withFinal: Timeline = {
  entries: [
    entry("n_findings", "2026-10-30", "due", { overdue: true }),
    entry("n_meeting", "2026-11-20", "pin", { final: true, shortfall_days: 2 }),
    entry("n_report", "2026-11-25", "pin"),
  ],
  end: "n_meeting",
};

/** The same dates with no final milestone. */
const withoutFinal: Timeline = {
  entries: withFinal.entries.map((each) => ({ ...each, final: false })),
};

describe("C13: the end anchor", () => {
  it("is the final milestone at its date, even with a pin after it", () => {
    expect(endAnchor(withFinal)).toEqual({ node: "n_meeting", date: "2026-11-20" });
    expect(timelineAxis(withFinal, today).anchor).toEqual({ node: "n_meeting", date: "2026-11-20" });
  });

  it("is none without a final milestone, and the axis still ends after the latest date", () => {
    const axis = timelineAxis(withoutFinal, today);
    expect(axis.anchor).toBeUndefined();
    expect(axis.end).toBe(dateOf(dayOf("2026-11-25") + AXIS_MARGIN_DAYS));
  });

  it("is none while the final milestone has no date", () => {
    const undated: Timeline = { entries: withoutFinal.entries, end: "n_meeting_later", undated: ["n_meeting_later"] };
    expect(endAnchor(undated)).toBeUndefined();
  });
});

describe("C13: the axis", () => {
  it("spans today and every date with a margin, today before the dates here", () => {
    const axis = timelineAxis(withFinal, today);
    expect(axis.start).toBe(dateOf(dayOf(today) - AXIS_MARGIN_DAYS));
    expect(axis.end).toBe(dateOf(dayOf("2026-11-25") + AXIS_MARGIN_DAYS));
    expect(axis.today).toBeCloseTo(AXIS_MARGIN_DAYS / (dayOf(axis.end) - dayOf(axis.start)));
  });

  it("spans an actual date before today", () => {
    const past: Timeline = { entries: [entry("n_meeting", "2026-09-01", "actual")] };
    const axis = timelineAxis(past, today);
    expect(axis.start).toBe(dateOf(dayOf("2026-09-01") - AXIS_MARGIN_DAYS));
    expect(axis.end).toBe(dateOf(dayOf(today) + AXIS_MARGIN_DAYS));
  });

  it("places each date in order along it, inside its ends", () => {
    const axis = timelineAxis(withFinal, today);
    const placed = withFinal.entries.map((each) => positionOf(axis, each.date));
    expect(placed.every((at) => at > 0 && at < 1)).toBe(true);
    expect([...placed].sort((a, b) => a - b)).toEqual(placed);
  });

  it("shows only a range's days around today, whatever dates lie beyond", () => {
    const month = timelineAxis(withFinal, today, [], rangeWindow("month", today));
    expect(month.start).toBe(dateOf(dayOf(today) - 3));
    expect(dayOf(month.end) - dayOf(month.start)).toBe(31);
    expect(rangeWindow("fit", today)).toBeUndefined();
  });

  const spans: { last: string; unit: string }[] = [
    { last: "2026-10-12", unit: "day" },
    { last: "2026-12-01", unit: "week" },
    { last: "2027-06-01", unit: "month" },
    { last: "2028-06-01", unit: "quarter" },
    { last: "2031-01-01", unit: "year" },
    { last: "2090-01-01", unit: "year" },
  ];
  for (const span of spans) {
    it(`ticks a span to ${span.last} by ${span.unit}, each at a step's start, few enough`, () => {
      const axis = timelineAxis({ entries: [entry("n_report", span.last, "pin")] }, today);
      expect(axis.unit).toBe(span.unit);
      expect(axis.ticks.length).toBeGreaterThan(0);
      expect(axis.ticks.length).toBeLessThanOrEqual(TICK_COUNT_MAX + 1);
      for (const tick of axis.ticks) {
        const date = new Date(`${tick.date}T00:00:00Z`);
        const starts = { day: true, week: date.getUTCDay() === 1, month: date.getUTCDate() === 1, quarter: date.getUTCDate() === 1 && date.getUTCMonth() % 3 === 0, year: date.getUTCDate() === 1 && date.getUTCMonth() === 0 };
        expect(starts[axis.unit]).toBe(true);
        expect(tick.at).toBeCloseTo(positionOf(axis, tick.date));
        expect(tick.at >= 0 && tick.at <= 1).toBe(true);
      }
    });
  }
});

describe("dates in words", () => {
  it("name the month and day, and the year only when it is not this one's", () => {
    expect(dateWords("2026-10-30", today)).toBe("Oct 30");
    expect(dateWords("2027-01-04", today)).toBe("Jan 4, 2027");
  });

  it("say how far off a date is, in plain days", () => {
    expect(dateAway("2026-10-30", today)).toBe("Oct 30, in 24 days");
    expect(dateAway("2026-10-07", today)).toBe("Oct 7, in 1 day");
    expect(dateAway(today, today)).toMatch(/, today$/);
    expect(dateAway("2026-10-02", today)).toBe("Oct 2, 4 days late");
  });
});
