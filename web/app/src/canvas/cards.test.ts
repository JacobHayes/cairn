// The zoomed-out title's size: as large as lets the whole of it show in its card's room, never below the reading size.
import { describe, expect, it } from "vitest";

import { CARD_WIDTH_PX, TITLE_FIT_MIN_PX, titleFitPx, titleRoom } from "./cards.ts";

describe("titleFitPx", () => {
  const room = titleRoom(CARD_WIDTH_PX, 112, { glyph: true, controls: 30 });

  it("sets a short title larger than a long one in the same room, and a title that cannot fit at the reading size", () => {
    const [short, forty, long] = [titleFitPx(8, room.width, room.height), titleFitPx(40, room.width, room.height), titleFitPx(200, room.width, room.height)];
    expect(short).toBeGreaterThan(forty);
    expect(forty).toBeGreaterThan(TITLE_FIT_MIN_PX);
    expect(long).toBe(TITLE_FIT_MIN_PX);
  });

  it("sets the same title larger in a taller card", () => {
    expect(titleFitPx(40, room.width, 200)).toBeGreaterThan(titleFitPx(40, room.width, 80));
  });
});
