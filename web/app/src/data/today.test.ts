// Date rollover: today in the deployment's zone, as the host computes it.
import { describe, expect, it } from "vitest";

import { todayIn } from "./today.ts";

describe("todayIn", () => {
  it.each([
    ["UTC", "2026-10-06T23:30:00Z", "2026-10-06"],
    ["Etc/GMT-2", "2026-10-06T23:30:00Z", "2026-10-07"],
    ["Etc/GMT+5", "2026-10-07T03:00:00Z", "2026-10-06"],
  ])("in %s at %s is %s", (zone, at, today) => {
    expect(todayIn(zone, new Date(at))).toBe(today);
  });
});
