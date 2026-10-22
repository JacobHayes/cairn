import { describe, expect, it } from "vitest";

import { CORNER, place, pointerRect, type PlaceOptions, type Rect } from "./place.ts";

const bounds: Rect = { left: 0, top: 0, width: 400, height: 300 };
const size = { width: 100, height: 40 };
const below: PlaceOptions = { side: "bottom", align: "center", gap: 6, margin: 8 };
const anchorAt = (left: number, top: number): Rect => ({ left, top, width: 60, height: 20 });

describe("place", () => {
  it("puts the box on its side, centred, a gap away", () => {
    expect(place(anchorAt(100, 100), size, bounds, below)).toMatchObject({ left: 80, top: 126, side: "bottom", room: 300 - 8 - 126 });
  });

  it("flips to the other side when its own lacks the room and the other has more", () => {
    expect(place(anchorAt(100, 260), size, bounds, below)).toMatchObject({ top: 260 - 6 - 40, side: "top" });
  });

  it("reports the room on the side it chose", () => {
    expect(place(anchorAt(100, 260), size, bounds, below).room).toBe(260 - 6 - 8);
  });

  it("stays when the other side has no more room", () => {
    // 20 above, 20 below: neither fits the box, so the preferred side keeps it (and it is slid).
    const tall = { width: 100, height: 400 };
    expect(place(anchorAt(100, 140), tall, bounds, below).side).toBe("bottom");
  });

  it("slides back to the margin at each edge", () => {
    expect(place(anchorAt(0, 100), size, bounds, below).left).toBe(8);
    expect(place(anchorAt(380, 100), size, bounds, below).left).toBe(400 - 8 - 100);
    expect(place(anchorAt(100, 0), size, bounds, { ...below, side: "top" }).top).toBe(26);
  });

  it("pins a box larger than the bounds to the leading margin", () => {
    expect(place(anchorAt(100, 100), { width: 900, height: 40 }, bounds, below).left).toBe(8);
  });

  it("aligns on the anchor's leading or trailing edge", () => {
    expect(place(anchorAt(100, 100), size, bounds, { ...below, align: "start" }).left).toBe(100);
    expect(place(anchorAt(100, 100), size, bounds, { ...below, align: "end" }).left).toBe(60);
  });

  it("places beside an anchor with the bottom edges level", () => {
    const beside: PlaceOptions = { side: "right", align: "end", gap: 0, margin: 12 };
    expect(place(anchorAt(20, 200), size, bounds, beside)).toMatchObject({ left: 80, top: 180, side: "right", room: 400 - 12 - 80 });
  });

  it("reports where the anchor's centre falls along the box's near edge, kept clear of the corners", () => {
    // Centred below the anchor: the centre is the box's middle.
    expect(place(anchorAt(100, 100), size, bounds, below).anchorOffset).toBe(50);
    // Aligned on the anchor's start, the centre is half the anchor in.
    expect(place(anchorAt(100, 100), size, bounds, { ...below, align: "start" }).anchorOffset).toBe(30);
    // An anchor beyond the end of a box slid to the margin: the notch stops at the corner.
    expect(place(anchorAt(380, 100), size, bounds, { ...below, align: "start" }).anchorOffset).toBe(100 - CORNER);
    // Beside the anchor the offset runs down the box's side.
    const beside: PlaceOptions = { side: "right", align: "center", gap: 0, margin: 12 };
    expect(place(anchorAt(20, 200), size, bounds, beside).anchorOffset).toBe(20);
    // A centre nearer the box's leading edge than the corner is held at the corner.
    expect(place(anchorAt(20, 200), size, bounds, { ...beside, align: "start" }).anchorOffset).toBe(CORNER);
  });

  it("takes a pointer as an anchor", () => {
    expect(pointerRect(10, 20)).toEqual({ left: 10, top: 20, width: 12, height: 20 });
  });
});
