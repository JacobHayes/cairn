// The menus' arrow keys (the View picker, `⋯`, the context menu): the arithmetic only; real
// focus is the browser tests'.
import { describe, expect, it } from "vitest";

import { menuFocusTarget } from "./menuKeys.ts";

describe("menuFocusTarget", () => {
  it.each([
    { key: "ArrowDown", at: -1, count: 5, to: 0 },
    { key: "ArrowUp", at: -1, count: 5, to: 4 },
    { key: "ArrowDown", at: 2, count: 5, to: 3 },
    { key: "ArrowUp", at: 2, count: 5, to: 1 },
    { key: "ArrowDown", at: 4, count: 5, to: 0 },
    { key: "ArrowUp", at: 0, count: 5, to: 4 },
    { key: "Home", at: 3, count: 5, to: 0 },
    { key: "End", at: 1, count: 5, to: 4 },
    { key: "ArrowDown", at: 0, count: 1, to: 0 },
    { key: "ArrowUp", at: 0, count: 1, to: 0 },
  ])("$key from $at of $count goes to $to", ({ key, at, count, to }) => {
    expect(menuFocusTarget(key, at, count)).toBe(to);
  });

  it("leaves every other key, and a menu with no items, alone", () => {
    expect(menuFocusTarget("a", 1, 5)).toBeUndefined();
    expect(menuFocusTarget("Enter", 1, 5)).toBeUndefined();
    expect(menuFocusTarget("ArrowDown", -1, 0)).toBeUndefined();
    expect(menuFocusTarget("End", -1, 0)).toBeUndefined();
  });
});
