// A receipt's lifetime: shown when a write lands, gone after its time, shown
// afresh and for a full time by the next save, and gone at once when cleared.
import { createActor } from "xstate";
import { describe, expect, it } from "vitest";

import { CountingClock } from "../data/fake.test-support.ts";
import { RECEIPT_MS, receiptMachine, receiptOf } from "./receipt.ts";

function receipt() {
  const clock = new CountingClock();
  const actor = createActor(receiptMachine, { clock }).start();
  return {
    clock,
    show: (warning?: string) => {
      actor.send({ type: "show", warning });
    },
    clear: () => {
      actor.send({ type: "clear" });
    },
    advance: (ms: number) => {
      clock.increment(ms);
    },
    shown: () => receiptOf(actor.getSnapshot()),
  };
}

describe("a receipt", () => {
  it("shows what the write caused and is gone after its time, not a millisecond before", () => {
    const { show, advance, shown } = receipt();
    show("Kickoff is now overdue.");
    expect(shown()).toEqual({ warning: "Kickoff is now overdue." });
    advance(RECEIPT_MS - 1);
    expect(shown()).toBeDefined();
    advance(1);
    expect(shown()).toBeUndefined();
  });

  it("starts its time afresh with the next save", () => {
    const { show, advance, shown } = receipt();
    show();
    advance(5000);
    show("Now stale.");
    advance(RECEIPT_MS - 1);
    expect(shown()).toEqual({ warning: "Now stale." });
    advance(1);
    expect(shown()).toBeUndefined();
  });

  it("is gone at once when cleared, and leaves no timer", () => {
    const { show, clear, shown, clock } = receipt();
    show();
    clear();
    expect(shown()).toBeUndefined();
    expect(clock.pending).toEqual([]);
  });
});
