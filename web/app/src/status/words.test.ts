// D8's words: each display state's word by kind, and that no two states share a glyph or a
// dot, so a chip that loses its word (far zoom, a narrow row) still tells them apart.
import { describe, expect, test } from "vitest";

import { DISPLAY_STATES, stateWord, statusDot, statusGlyph, statusWord } from "./words.ts";

describe("display words", () => {
  test("a decision is to decide or decided, a milestone reached, a group not started", () => {
    expect(statusWord("ready", "decision")).toBe("to decide");
    expect(statusWord("done", "decision")).toBe("decided");
    expect(statusWord("done", "milestone")).toBe("reached");
    expect(statusWord("ready", "group")).toBe("not started");
    expect(statusWord("ready", "action")).toBe("ready");
    expect(statusWord("not_relevant", "decision")).toBe("not relevant");
    expect(stateWord("conditional")).toBe("conditional");
  });

  test("no stored-state word is a display word", () => {
    const words = DISPLAY_STATES.map((state) => stateWord(state));
    for (const stored of ["open", "todo", "pending", "derived"]) {
      expect(words).not.toContain(stored);
    }
  });

  test("every state has its own glyph and its own dot", () => {
    expect(new Set(DISPLAY_STATES.map(statusGlyph)).size).toBe(DISPLAY_STATES.length);
    expect(new Set(DISPLAY_STATES.map((state) => JSON.stringify(statusDot(state)))).size).toBe(DISPLAY_STATES.length);
  });
});
