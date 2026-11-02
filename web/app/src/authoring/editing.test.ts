// A form's kept draft is unsent only when it differs from how it opened.
import { describe, expect, it } from "vitest";

import { unsent } from "./editing.ts";

describe("a kept editing draft", () => {
  it("is unsent once its value differs from how it opened, and not before", () => {
    expect(unsent({ value: { title: "A" }, base: 1, opened: { title: "A" } })).toBe(false);
    expect(unsent({ value: { title: "AB" }, base: 1, opened: { title: "A" } })).toBe(true);
  });
});
