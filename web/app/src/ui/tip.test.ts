// The tooltip's timing: a rest before the first, none between neighbours, a beat of grace to reach
// the tip, focus and taps at once, and a dismissed thing quiet until the pointer comes back.
import { createActor } from "xstate";
import { describe, expect, it } from "vitest";

import { CountingClock } from "../data/fake.test-support.ts";
import { HOVER_LEAVE_MS, TIP_SHOW_MS, TIP_SKIP_MS, tipMachine, tipShown, tipTarget, type TipEvent } from "./tip.ts";

const [a, b] = [{ id: "a" }, { id: "b" }];

function tips() {
  const clock = new CountingClock();
  const actor = createActor(tipMachine, { clock }).start();
  return {
    send: (event: TipEvent) => {
      actor.send(event);
    },
    advance: (ms: number) => {
      clock.increment(ms);
    },
    /** The id of the tip being drawn, if any. */
    anchor: () => tipTarget(actor.getSnapshot())?.target.box,
    shown: () => (tipShown(actor.getSnapshot()) ? tipTarget(actor.getSnapshot())?.target.id : undefined),
  };
}

describe("the tooltip's timing by hover", () => {
  it("shows after the pointer has rested, and a move restarts the rest", () => {
    const { send, advance, shown } = tips();
    send({ type: "hover", target: a });
    advance(TIP_SHOW_MS - 1);
    expect(shown()).toBeUndefined();
    send({ type: "hover", target: a });
    advance(TIP_SHOW_MS - 1);
    expect(shown()).toBeUndefined();
    advance(1);
    expect(shown()).toBe("a");
  });

  it("shows nothing when the pointer leaves before the rest is over", () => {
    const { send, advance, shown } = tips();
    send({ type: "hover", target: a });
    send({ type: "unhover" });
    advance(TIP_SHOW_MS * 2);
    expect(shown()).toBeUndefined();
  });

  it("moves to a neighbour at once, while one is open and within the skip window after it goes", () => {
    const { send, advance, shown } = tips();
    send({ type: "hover", target: a });
    advance(TIP_SHOW_MS);
    send({ type: "hover", target: b });
    expect(shown()).toBe("b");
    send({ type: "unhover" });
    advance(HOVER_LEAVE_MS);
    advance(TIP_SKIP_MS - 1);
    send({ type: "hover", target: a });
    expect(shown()).toBe("a");
    send({ type: "unhover" });
    advance(HOVER_LEAVE_MS);
    advance(TIP_SKIP_MS);
    send({ type: "hover", target: b });
    expect(shown()).toBeUndefined();
    advance(TIP_SHOW_MS);
    expect(shown()).toBe("b");
  });

  it("keeps the tip when the pointer comes back in the beat, and drops it when it does not", () => {
    const { send, advance, shown } = tips();
    send({ type: "hover", target: a });
    advance(TIP_SHOW_MS);
    send({ type: "unhover" });
    advance(HOVER_LEAVE_MS - 1);
    expect(shown()).toBe("a");
    send({ type: "hover", target: a });
    advance(HOVER_LEAVE_MS * 3);
    expect(shown()).toBe("a");
    send({ type: "unhover" });
    advance(HOVER_LEAVE_MS);
    expect(shown()).toBeUndefined();
  });

});

describe("the tooltip opened by focus or a tap", () => {
  it("opens on focus at once and a pointer leaving does not close it", () => {
    const { send, advance, shown } = tips();
    send({ type: "focus", target: a });
    expect(shown()).toBe("a");
    send({ type: "hover", target: b });
    send({ type: "unhover" });
    advance(HOVER_LEAVE_MS * 4);
    expect(shown()).toBe("a");
    send({ type: "blur" });
    expect(shown()).toBeUndefined();
  });

  it("toggles a tapped tip, and a pointer leaving does not close it", () => {
    const { send, advance, shown } = tips();
    send({ type: "tap", target: a });
    expect(shown()).toBe("a");
    send({ type: "unhover" });
    advance(HOVER_LEAVE_MS * 4);
    expect(shown()).toBe("a");
    send({ type: "tap", target: a });
    expect(shown()).toBeUndefined();
  });

});

describe("a thing the tooltip leaves quiet", () => {
  it("keeps a dismissed thing quiet until the pointer has left it and come back", () => {
    const { send, advance, shown } = tips();
    send({ type: "hover", target: a });
    advance(TIP_SHOW_MS);
    send({ type: "dismiss" });
    expect(shown()).toBeUndefined();
    send({ type: "hover", target: a });
    advance(TIP_SHOW_MS * 2);
    expect(shown()).toBeUndefined();
    send({ type: "unhover" });
    send({ type: "hover", target: a });
    advance(TIP_SHOW_MS);
    expect(shown()).toBe("a");
  });
});

describe("the tooltip of a thing with no box", () => {
  it("opens where the pointer rested, not where it first came onto the thing", () => {
    const { send, advance, anchor } = tips();
    send({ type: "hover", target: { id: "line", box: { left: 10, top: 10, width: 0, height: 0 } } });
    advance(TIP_SHOW_MS - 1);
    send({ type: "hover", target: { id: "line", box: { left: 200, top: 40, width: 0, height: 0 } } });
    advance(TIP_SHOW_MS);
    expect(anchor()).toEqual({ left: 200, top: 40, width: 0, height: 0 });
  });

  it("goes with its owner when the owner drops it, and not when another thing's owner does", () => {
    const { send, advance, shown } = tips();
    send({ type: "hover", target: a });
    advance(TIP_SHOW_MS);
    send({ type: "drop", id: "b" });
    expect(shown()).toBe("a");
    send({ type: "drop", id: "a" });
    expect(shown()).toBeUndefined();
  });
});
