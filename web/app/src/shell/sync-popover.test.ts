// The sync chip's popover (design 9): pinned by a click, peeked by hover or focus, a beat's
// grace after the pointer leaves, and closed at once by Esc, a press outside or a resize.
import { createActor } from "xstate";
import { describe, expect, it } from "vitest";

import { CountingClock } from "../data/fake.test-support.ts";
import { HOVER_LEAVE_MS } from "../ui/tip.ts";
import { popoverAnchor, popoverPinned, popoverShown, syncPopover, type Anchor, type PopoverEvent } from "./sync-popover.ts";

const here: Anchor = { top: 20, left: 30, width: 10, height: 10 };
const there: Anchor = { top: 40, left: 50, width: 10, height: 10 };

function popover() {
  const clock = new CountingClock();
  const actor = createActor(syncPopover, { clock }).start();
  return {
    clock,
    send: (event: PopoverEvent) => {
      actor.send(event);
    },
    advance: (ms: number) => {
      clock.increment(ms);
    },
    state: () => {
      const snapshot = actor.getSnapshot();
      return { shown: popoverShown(snapshot), pinned: popoverPinned(snapshot) };
    },
    anchor: () => popoverAnchor(actor.getSnapshot()),
  };
}

describe("the sync chip's popover", () => {
  it("is peeked by hover and not pinned, and goes a beat after the pointer leaves", () => {
    const { send, advance, state } = popover();
    send({ type: "peek", anchor: here });
    expect(state()).toEqual({ shown: true, pinned: false });
    send({ type: "unpeek" });
    advance(HOVER_LEAVE_MS - 1);
    expect(state().shown).toBe(true);
    advance(1);
    expect(state().shown).toBe(false);
  });

  it("stays when the pointer comes back within the beat, past the first leave's time", () => {
    const { send, advance, state } = popover();
    send({ type: "peek", anchor: here });
    send({ type: "unpeek" });
    advance(100);
    send({ type: "peek", anchor: here });
    advance(HOVER_LEAVE_MS);
    expect(state().shown).toBe(true);
  });

  it("gives a second leave its own full beat", () => {
    const { send, advance, state } = popover();
    send({ type: "peek", anchor: here });
    send({ type: "unpeek" });
    advance(100);
    send({ type: "unpeek" });
    advance(HOVER_LEAVE_MS - 1);
    expect(state().shown).toBe(true);
    advance(1);
    expect(state().shown).toBe(false);
  });

});

describe("the sync chip's popover, pinned and closed", () => {
  it("stays when pinned whatever the pointer does, and a click unpins it", () => {
    const { send, advance, state } = popover();
    send({ type: "pin", anchor: here });
    send({ type: "peek", anchor: here });
    send({ type: "unpeek" });
    advance(HOVER_LEAVE_MS);
    expect(state()).toEqual({ shown: true, pinned: true });
    send({ type: "pin", anchor: here });
    expect(state()).toEqual({ shown: false, pinned: false });
  });

  it("stays peeked when unpinned with the pointer still on the chip", () => {
    const { send, state } = popover();
    send({ type: "peek", anchor: here });
    send({ type: "pin", anchor: here });
    send({ type: "pin", anchor: here });
    expect(state()).toEqual({ shown: true, pinned: false });
  });

  it("closes both at once, and leaves no beat to run afterwards", () => {
    const { send, advance, state, clock } = popover();
    send({ type: "pin", anchor: here });
    send({ type: "peek", anchor: here });
    send({ type: "unpeek" });
    send({ type: "close" });
    expect(state()).toEqual({ shown: false, pinned: false });
    expect(clock.pending).toEqual([]);
    // Coming back after the close starts a new peek that the old beat cannot end.
    send({ type: "peek", anchor: here });
    advance(HOVER_LEAVE_MS);
    expect(state().shown).toBe(true);
  });

  it("opens where the last peek or pin said", () => {
    const { send, anchor } = popover();
    send({ type: "peek", anchor: here });
    expect(anchor()).toEqual(here);
    send({ type: "pin", anchor: there });
    expect(anchor()).toEqual(there);
  });
});
