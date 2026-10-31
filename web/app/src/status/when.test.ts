// F1, F3: dates in words, one voice for every surface: a distance while a date is near, a date
// after that, late once it has passed, and one reading of where a date comes from.
import { describe, expect, it } from "vitest";

import { dateParts, dateWords, daysUntil, dueWords, lateWords, localWall, momentFrom, NEAR_DAYS, originWord, relativeDays, timeWords, whenWords } from "./when.ts";

const today = "2026-10-06";

describe("a calendar date in words", () => {
  it("names the month and day, and the year only when it is not this one's", () => {
    expect(dateWords("2026-10-30", today)).toBe("Oct 30");
    expect(dateWords("2027-01-04", today)).toBe("Jan 4, 2027");
    expect(dateWords("2026-10-30")).toBe("Oct 30, 2026");
  });
});

describe("when a node is due", () => {
  it("is late by the days the engine counts, for work and decisions alike, and late today for none", () => {
    expect(dueWords("2026-10-02", today, false, 4)).toBe(dueWords("2026-10-02", today, true, 4));
    expect(dueWords("2026-10-02", today, false, 4)).toContain("4");
    expect(dueWords("2026-10-06T08:15+00:00", today, false, 0)).toContain("today");
  });

  it("is a distance within two weeks and the date beyond, for work", () => {
    expect(dueWords("2026-10-09", today, false, undefined)).toContain("3");
    expect(dueWords("2026-10-09", today, false, undefined)).not.toContain("Oct");
    const far = dateWords("2026-11-30", today);
    expect(dueWords("2026-11-30", today, false, undefined)).toContain(far);
    expect(daysUntil("2026-10-20", today)).toBe(NEAR_DAYS);
    expect(dueWords("2026-10-20", today, false, undefined)).not.toContain("Oct");
    expect(dueWords("2026-10-21", today, false, undefined)).toContain("Oct");
  });

  it("names the date for a decision at any distance, so it reads the same near and far", () => {
    expect(dueWords("2026-10-09", today, true, undefined)).toContain("Oct 9");
    expect(dueWords("2026-11-30", today, true, undefined)).toContain("Nov 30");
  });
});

describe("a milestone's date as a reader weighs it", () => {
  it("says an actual date was done, never late", () => {
    expect(whenWords("2026-10-01", "actual", today, undefined)).toContain("done");
    expect(whenWords("2026-10-01", "actual", today, undefined)).not.toContain("late");
  });

  it.each(["pin", "due"] as const)("says a %s date the engine calls late is late, and one ahead how far off it is, with the date either way", (origin) => {
    expect(whenWords("2026-10-01", origin, today, 5)).toBe(`${lateWords(5)} (${dateWords("2026-10-01", today)})`);
    expect(whenWords("2026-10-30", origin, today, undefined)).toBe(`${relativeDays(24)} (${dateWords("2026-10-30", today)})`);
    expect(whenWords(today, origin, today, undefined)).toContain(relativeDays(0));
  });
});

describe("where a date comes from", () => {
  it("is said one way whichever vocabulary the engine used for it", () => {
    expect(originWord("derived")).toBe(originWord("due"));
    expect(new Set([originWord("actual"), originWord("pin"), originWord("due")]).size).toBe(3);
  });
});

// A9: a moment is shown and entered on the viewer's clock and stored as the same instant in the deployment's zone.
describe("a time of day", () => {
  // 23:00 in New York, which is 04:00 the next day in London and 20:00 the same day in Los Angeles.
  const moment = "2026-10-10T23:00-04:00";
  const instant = Date.parse(moment);

  it("is written in the viewer's zone and hour cycle", () => {
    expect(timeWords(instant, "Europe/London", "en-GB")).toBe("04:00");
    expect(timeWords(instant, "America/Los_Angeles", "en-US")).toMatch(/^8:00\sPM$/);
  });

  it("is read on the viewer's clock, and a day has none", () => {
    expect(localWall(moment, "Europe/London")).toEqual({ day: "2026-10-11", time: "04:00" });
    expect(localWall(moment, "America/Los_Angeles")).toEqual({ day: "2026-10-10", time: "20:00" });
    expect(localWall("2026-10-10", "Europe/London")).toBeUndefined();
  });

  it("follows the viewer's date in words, while a day is not converted", () => {
    // The tests run in UTC, where the moment is 03:00 on Oct 11.
    expect(dateWords(moment, today)).toBe(`${dateWords("2026-10-11", today)}, ${timeWords(instant)}`);
    expect(dateWords("2026-10-10", today)).not.toBe(dateWords("2026-10-11", today));
  });

  it("is named by its date and time, never a distance in days, which the viewer's clock need not share", () => {
    for (const on of ["2026-10-09", "2026-10-10"]) {
      expect(dueWords(moment, on, false, undefined)).toBe(`Due ${dateWords(moment, on)}`);
      expect(dueWords(moment, on, true, undefined)).toBe(`Decide by ${dateWords(moment, on)}`);
      expect(whenWords(moment, "pin", on, undefined)).toBe(dateWords(moment, on));
    }
    expect(dueWords("2026-10-10", "2026-10-09", false, undefined)).not.toContain(dateWords("2026-10-10", "2026-10-09"));
  });

  it("is entered on the viewer's clock as the same instant in the deployment's zone, with its offset then", () => {
    expect(momentFrom("America/New_York", "2026-10-11", "04:00", "Europe/London")).toBe(moment);
    expect(momentFrom("America/New_York", "2026-10-10", "20:00", "America/Los_Angeles")).toBe(moment);
    expect(momentFrom("Europe/London", "2026-12-10", "08:15", "Europe/London")).toBe("2026-12-10T08:15+00:00");
    expect(momentFrom("Asia/Kolkata", "2026-10-10", "08:15", "UTC")).toBe("2026-10-10T13:45+05:30");
  });

  it("reads a repeated wall time as its first and a skipped one as the time on", () => {
    expect(momentFrom("Europe/London", "2026-10-25", "01:30", "Europe/London")).toBe("2026-10-25T01:30+01:00");
    expect(momentFrom("Europe/London", "2026-03-29", "01:30", "Europe/London")).toBe("2026-03-29T02:30+01:00");
    expect(momentFrom("UTC", "2026-10-25", "01:30", "Europe/London")).toBe("2026-10-25T00:30+00:00");
  });
});

describe("the dates and times in words", () => {
  it("are cut out of what dateWords, dueWords and whenWords say, and nothing else is", () => {
    const said = [dateWords("2026-10-06", "2026-10-01"), dateWords("2027-10-06", "2026-10-01"), dateWords("2026-10-06T08:15+00:00", "2026-10-01")];
    for (const words of said) {
      expect(dateParts(words)).toEqual([{ text: words, date: true }]);
    }
    const due = dueWords("2026-10-02T08:15+00:00", "2026-10-01", false, undefined);
    expect(dateParts(due).map((part) => part.date)).toEqual([false, true]);
    expect(dateParts(due)[1]?.text).toBe(dateWords("2026-10-02T08:15+00:00", "2026-10-01"));
    const when = whenWords("2026-10-20", undefined, "2026-10-01", undefined);
    expect(dateParts(when).filter((part) => part.date).map((part) => part.text)).toEqual([dateWords("2026-10-20", "2026-10-01")]);
    expect(dateParts(lateWords(3))).toEqual([{ text: lateWords(3), date: false }]);
  });

  it("find a time in either hour cycle", () => {
    for (const [zone, locale] of [["Europe/London", "en-GB"], ["America/Los_Angeles", "en-US"], ["Asia/Kolkata", "en-IN"]] as const) {
      const time = timeWords(Date.parse("2026-10-10T23:00-04:00"), zone, locale);
      expect(dateParts(`Snoozed until ${time} today`)).toEqual([{ text: "Snoozed until ", date: false }, { text: time, date: true }, { text: " today", date: false }]);
      expect(dateParts(`Oct 6, ${time}`)).toEqual([{ text: `Oct 6, ${time}`, date: true }]);
    }
  });
});
