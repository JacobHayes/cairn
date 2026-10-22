// A zoomed-out container head's title: as large as lets the whole of it show, and the chip only when it costs the title nothing.
import { describe, expect, it } from "vitest";

import { CARD_WIDTH_PX, TITLE_FIT_MIN_PX, headChipFits, headTitleBox, titleFitPx, titleLines, titleLinesOf, TITLE_LINES_MAX } from "./cards.ts";

describe("titleFitPx", () => {
  const box = headTitleBox(CARD_WIDTH_PX, 112, false);
  const words = (count: number) => "word ".repeat(count).trim();

  it("sets a short title larger than a long one in the same room, and a title that cannot fit at the least size", () => {
    const [short, forty, long] = [titleFitPx("Short", box.width, box.height), titleFitPx(words(8), box.width, box.height), titleFitPx(words(200), box.width, box.height)];
    expect(short).toBeGreaterThan(forty);
    expect(forty).toBeGreaterThan(TITLE_FIT_MIN_PX);
    expect(long).toBe(TITLE_FIT_MIN_PX);
  });

  it("sets the same title larger in a taller card", () => {
    expect(titleFitPx(words(8), box.width, 200)).toBeGreaterThan(titleFitPx(words(8), box.width, 80));
  });

  it("shrinks a title until its longest word fits a line, so no word breaks", () => {
    const size = titleFitPx("Stakeholders", box.width, box.height);
    expect(box.width / size).toBeGreaterThanOrEqual(0.78 + 11 * 0.58);
  });

  it("sets capitals smaller than lowercase of the same length", () => {
    expect(titleFitPx("WWWW WWWW WWWW", box.width, box.height)).toBeLessThan(titleFitPx("wwww wwww wwww", box.width, box.height));
  });

  it("counts whole lines of a title's size in a room", () => {
    expect(titleLines(20, 70)).toBe(3);
    expect(titleLines(60, 10)).toBe(1);
  });
});

describe("titleLinesOf", () => {
  it("counts the lines a title wraps to between its words, capitals taking more room than lowercase, never more than the most", () => {
    const [short, caps, lower, long, unbroken] = [titleLinesOf("Short title"), titleLinesOf("WIDE WIDE WIDE WIDE WIDE"), titleLinesOf("wide wide wide wide wide"), titleLinesOf("word ".repeat(40)), titleLinesOf("x".repeat(80))];
    expect(short).toBe(1);
    expect(caps).toBeGreaterThan(lower);
    expect(long).toBe(TITLE_LINES_MAX);
    expect(unbroken).toBe(TITLE_LINES_MAX);
  });
});

describe("headChipFits", () => {
  const wide = headTitleBox(600, 80, false);

  it("keeps the chip while the title keeps its size and lines, and drops it when the title would shrink or wrap", () => {
    expect(headChipFits("Setup", 4, wide, 0.3)).toBe(true);
    expect(headChipFits("Evaluate the analytics vendor stage", 11, headTitleBox(CARD_WIDTH_PX, 80, false), 0.3)).toBe(false);
  });

  it("drops the chip beside a title whose one word is wider than the head, so the cut word keeps the room", () => {
    expect(headChipFits("Unbroken".concat("x".repeat(240)), 7, wide, 0.3)).toBe(false);
  });
});
