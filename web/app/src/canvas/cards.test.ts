// The zoomed-out title's size: as large as lets the whole of it show in its card's room, never below the reading size.
import { describe, expect, it } from "vitest";

import { CARD_WIDTH_PX, TITLE_FIT_MIN_PX, titleFitPx, titleLines, titleRoom, titleWordOverflows } from "./cards.ts";

describe("titleFitPx", () => {
  const room = titleRoom(CARD_WIDTH_PX, 112, { glyph: true, controls: 30 });
  const words = (count: number) => "word ".repeat(count).trim();

  it("sets a short title larger than a long one in the same room, and a title that cannot fit at the reading size", () => {
    const [short, forty, long] = [titleFitPx("Short", room.width, room.height), titleFitPx(words(8), room.width, room.height), titleFitPx(words(40), room.width, room.height)];
    expect(short).toBeGreaterThan(forty);
    expect(forty).toBeGreaterThan(TITLE_FIT_MIN_PX);
    expect(long).toBe(TITLE_FIT_MIN_PX);
  });

  it("sets the same title larger in a taller card", () => {
    expect(titleFitPx(words(8), room.width, 200)).toBeGreaterThan(titleFitPx(words(8), room.width, 80));
  });

  it("shrinks a title until its longest word fits a line, so no word breaks", () => {
    const size = titleFitPx("Stakeholders", room.width, room.height);
    expect(room.width / size).toBeGreaterThanOrEqual(0.78 + 11 * 0.58);
  });

  it("sets capitals smaller than lowercase of the same length, and flags a word that cannot fit at the reading size", () => {
    expect(titleFitPx("WWWW WWWW WWWW", room.width, room.height)).toBeLessThan(titleFitPx("wwww wwww wwww", room.width, room.height));
    expect(titleWordOverflows("Supercalifragilisticexpialidocious", room.width)).toBe(true);
    expect(titleWordOverflows("Stakeholders", room.width)).toBe(false);
  });

  it("counts whole lines of a title's size in a room", () => {
    expect(titleLines(20, 70)).toBe(3);
    expect(titleLines(60, 10)).toBe(1);
  });
});
