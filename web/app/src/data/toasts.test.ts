// The toast's lifetime (design 9.7): it goes after its time, the time waits while the pointer
// is on it and starts again in full when the pointer leaves, and a new toast takes the place of
// the one on screen with a full time of its own.
import { createActor } from "xstate";
import { describe, expect, it } from "vitest";

import { CountingClock } from "./fake.test-support.ts";
import { TOAST_MS, toastMachine, toastOf, type Toast } from "./toasts.ts";

function toasts() {
  const clock = new CountingClock();
  const actor = createActor(toastMachine, { clock }).start();
  return {
    clock,
    show: (text: string, tone: Toast["tone"] = "confirm") => {
      actor.send({ type: "show", text, tone });
    },
    hover: () => {
      actor.send({ type: "hover" });
    },
    leave: () => {
      actor.send({ type: "leave" });
    },
    advance: (ms: number) => {
      clock.increment(ms);
    },
    text: () => toastOf(actor.getSnapshot())?.text,
    id: () => toastOf(actor.getSnapshot())?.id,
  };
}

describe("the toast", () => {
  it.each([
    ["confirm", TOAST_MS.confirm],
    ["problem", TOAST_MS.problem],
  ] as const)("is gone after its time, a %s's by the tone, and not a millisecond before", (tone, ms) => {
    const { show, advance, text } = toasts();
    show("Exported", tone);
    advance(ms - 1);
    expect(text()).toBe("Exported");
    advance(1);
    expect(text()).toBeUndefined();
  });

  it("stays while the pointer is on it, and goes a full time after the pointer leaves", () => {
    const { show, hover, leave, advance, text, clock } = toasts();
    show("Exported");
    advance(2000);
    hover();
    advance(18_000);
    expect(text()).toBe("Exported");
    expect(clock.pending).toEqual([]);
    leave();
    advance(TOAST_MS.confirm - 1);
    expect(text()).toBe("Exported");
    advance(1);
    expect(text()).toBeUndefined();
  });

  it("is replaced by the next, which takes a full time of its own and not what was left of the first", () => {
    const { show, advance, text, id } = toasts();
    show("First");
    const first = id();
    advance(2000);
    show("Second");
    expect(text()).toBe("Second");
    expect(id()).not.toBe(first);
    advance(TOAST_MS.confirm - 1);
    expect(text()).toBe("Second");
    advance(1);
    expect(text()).toBeUndefined();
  });

  it("is replaced while the pointer holds the first, and the pointer holds the new one until it leaves", () => {
    const { show, hover, leave, advance, text, id } = toasts();
    show("First");
    hover();
    const first = id();
    show("Second");
    expect(text()).toBe("Second");
    expect(id()).not.toBe(first);
    advance(60_000);
    expect(text()).toBe("Second");
    leave();
    advance(TOAST_MS.confirm);
    expect(text()).toBeUndefined();
  });

  it("shows again after it has gone", () => {
    const { show, advance, text } = toasts();
    show("First");
    advance(TOAST_MS.confirm);
    show("Again");
    expect(text()).toBe("Again");
  });
});
