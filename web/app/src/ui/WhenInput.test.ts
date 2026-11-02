// A date with a time of day (A9), as the date and time fields join them: the value the engine stores.
import { describe, expect, it } from "vitest";

import { joined, timeTyped } from "./WhenInput.tsx";

describe("a date with a time of day", () => {
  it("puts a time typed before any date on today, so the value holds what is typed", () => {
    expect(timeTyped("UTC", "", "14:30", "2026-11-02")).toBe("2026-11-02T14:30+00:00");
    expect(timeTyped("UTC", "2026-10-10", "14:30", "2026-11-02")).toBe("2026-10-10T14:30+00:00");
  });

  it("keeps the time when the date changes, leaves the day when the time is cleared, and clears both with the date", () => {
    expect(joined("UTC", "2026-11-03", "14:30")).toBe("2026-11-03T14:30+00:00");
    expect(joined("UTC", "2026-11-03", "")).toBe("2026-11-03");
    expect(joined("UTC", "", "14:30")).toBe("");
  });
});
