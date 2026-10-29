// A form's kept draft is read back only when it is the shape the form reads now, and unsent only when
// it differs from how it opened.
import { describe, expect, it } from "vitest";

import { isEditing, unsent } from "./editing.ts";
import { isKept } from "./NodeForm.tsx";

describe("a kept editing draft", () => {
  it("is unsent once its value differs from how it opened, and not before", () => {
    expect(unsent({ value: { title: "A" }, base: 1, opened: { title: "A" } })).toBe(false);
    expect(unsent({ value: { title: "AB" }, base: 1, opened: { title: "A" } })).toBe(true);
  });

  it("is read only in the current shape", () => {
    expect(isEditing({ value: { id: "a" }, base: 3, opened: { id: "a" } })).toBe(true);
    expect(isEditing({ slot: { id: "a" }, base: 3, opened: { id: "a" } })).toBe(false);
    expect(isEditing({ form: { title: "x" }, base: 3, opened: { title: "x" } })).toBe(false);
  });
});

describe("a kept node form draft", () => {
  it("is read only when its date rules are the editor's parts", () => {
    expect(isKept({ edits: { title: "T" }, base: 1 })).toBe(true);
    expect(isKept({ edits: { due_by: { direction: "before", sources: ["n_a"], offset: "2" } }, base: 1 })).toBe(true);
    expect(isKept({ edits: { due_by: { before: "n_a", offset: 2 } }, base: 1 })).toBe(false);
    expect(isKept({ base: 1 })).toBe(false);
  });
});
