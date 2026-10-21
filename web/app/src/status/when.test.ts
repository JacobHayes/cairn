// F1, F3: dates in words, one voice for every surface: a distance while a date is near, a date
// after that, late once it has passed, and one reading of where a date comes from.
import { describe, expect, it } from "vitest";

import { dateWords, daysUntil, dueWords, NEAR_DAYS, originWord, relativeDays, whenWords } from "./when.ts";

const today = "2026-10-06";

describe("a calendar date in words", () => {
  it("names the month and day, and the year only when it is not this one's", () => {
    expect(dateWords("2026-10-30", today)).toBe("Oct 30");
    expect(dateWords("2027-01-04", today)).toBe("Jan 4, 2027");
    expect(dateWords("2026-10-30")).toBe("Oct 30, 2026");
  });
});

describe("when a node is due", () => {
  it("is late by the days gone, for work and decisions alike", () => {
    expect(dueWords("2026-10-02", today)).toBe(dueWords("2026-10-02", today, true));
    expect(dueWords("2026-10-02", today)).toContain("4");
  });

  it("is a distance within two weeks and the date beyond, for work", () => {
    expect(dueWords("2026-10-09", today)).toContain("3");
    expect(dueWords("2026-10-09", today)).not.toContain("Oct");
    const far = dateWords("2026-11-30", today);
    expect(dueWords("2026-11-30", today)).toContain(far);
    expect(daysUntil("2026-10-20", today)).toBe(NEAR_DAYS);
    expect(dueWords("2026-10-20", today)).not.toContain("Oct");
    expect(dueWords("2026-10-21", today)).toContain("Oct");
  });

  it("names the date for a decision at any distance, so it reads the same near and far", () => {
    expect(dueWords("2026-10-09", today, true)).toContain("Oct 9");
    expect(dueWords("2026-11-30", today, true)).toContain("Nov 30");
  });
});

describe("a milestone's date as a reader weighs it", () => {
  it("says an actual date was done, never late", () => {
    expect(whenWords("2026-10-01", "actual", today)).toContain("done");
    expect(whenWords("2026-10-01", "actual", today)).not.toContain("late");
  });

  it.each(["pin", "due"] as const)("says a %s date that has gone by is late, and one ahead how far off it is, with the date either way", (origin) => {
    expect(whenWords("2026-10-01", origin, today)).toContain("late");
    expect(whenWords("2026-10-30", origin, today)).toBe(`${relativeDays(24)} (${dateWords("2026-10-30", today)})`);
    expect(whenWords(today, origin, today)).toContain(relativeDays(0));
  });
});

describe("where a date comes from", () => {
  it("is said one way whichever vocabulary the engine used for it", () => {
    expect(originWord("derived")).toBe(originWord("due"));
    expect(new Set([originWord("actual"), originWord("pin"), originWord("due")]).size).toBe(3);
  });
});
