// Where Tab goes between the callout's controls and the action area's: the order is the callout,
// then the area, then the callout again, and from anywhere else focus comes back to the callout.
import { describe, expect, it } from "vitest";

import { tabDestination } from "./focus.ts";

const callout = ["back", "next", "skip"];
const area = ["yes", "no", "save"];
const go = (active: string | undefined, shift: boolean, groups = { callout, area }) => tabDestination({ ...groups, active, shift });

describe("Tab", () => {
  it("wraps from the callout's last control to the area's first, and from the area's last back to the callout's first", () => {
    expect(go("skip", false)).toBe("yes");
    expect(go("save", false)).toBe("back");
  });

  it("moves as the browser does inside each group", () => {
    expect(go("back", false)).toBeUndefined();
    expect(go("next", false)).toBeUndefined();
    expect(go("yes", false)).toBeUndefined();
    expect(go("no", false)).toBeUndefined();
  });

  it("wraps the other way with Shift", () => {
    expect(go("back", true)).toBe("save");
    expect(go("yes", true)).toBe("skip");
    expect(go("next", true)).toBeUndefined();
    expect(go("save", true)).toBeUndefined();
  });

  it("has the callout alone loop on itself when the area has nothing to focus", () => {
    const alone = { callout, area: [] };
    expect(go("skip", false, alone)).toBe("back");
    expect(go("back", true, alone)).toBe("skip");
  });

  it("brings a focus that is anywhere else back into the callout, from either end", () => {
    expect(go(undefined, false)).toBe("back");
    expect(go("elsewhere", false)).toBe("back");
    expect(go("elsewhere", true)).toBe("skip");
  });
});
