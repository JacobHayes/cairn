// What the sync chip says (design 9): the precedence among its states and the data each case
// carries (summarize), the words and colour of a case (syncWords), and the scheduler that wakes
// at each time threshold that keeps the chip from flickering (syncMachine, on a simulated clock).
import { createActor } from "xstate";
import { describe, expect, it } from "vitest";

import type { SaveEvent } from "./activity.ts";
import { CountingClock } from "./fake.test-support.ts";
import type { Lag } from "./keyed-read.ts";
import {
  SYNC_BEHIND_AFTER_MS,
  SYNC_SAVED_MS,
  SYNC_SHOW_AFTER_MS,
  nextDeadline,
  savedAnnouncement,
  settled,
  summarize,
  syncInputOf,
  syncMachine,
  syncSummary,
  syncWords,
  type Problem,
  type SyncEvent,
  type SyncInput,
  type SyncReading,
  type SyncState,
  type SyncTiming,
  type SyncTone,
} from "./sync.ts";

const quiet: SyncInput = {
  now: 100_000,
  stream: "live",
  online: true,
  skew: false,
  problems: [],
  inFlight: undefined,
  lag: undefined,
  saved: undefined,
  revision: 42,
};

const problem = (kind: Problem["kind"], label = "Draft test plan"): Problem => ({ id: `p_${kind}_${label}`, kind, message: "no", detail: undefined, label, address: undefined, retry: false });
const lag = (since: number, failed = false): Lag => ({ subject: "journey:j_one", shown: 42, announced: 43, since, failed });
const save = (id: number, warning?: string): SaveEvent => ({ id, at: new Date("2026-10-06T14:02:11"), text: "Renamed", warning, unlocked: undefined, address: undefined });

describe("summarize", () => {
  it("puts the first state of the precedence on top of the others that hold", () => {
    let held: Partial<SyncInput> = {
      problems: [problem("conflict"), problem("rejected")],
      skew: true,
      online: false,
      stream: "reconnecting",
      lag: lag(90_000),
      inFlight: { count: 1, since: 90_000 },
      saved: { at: 99_500, time: "14:02:11" },
    };
    // Take away what holds, top of the precedence first: the next state shows.
    const drops: Partial<SyncInput>[] = [
      { problems: [problem("rejected")] },
      { problems: [] },
      { skew: false },
      { online: true },
      { stream: "live" },
      { lag: undefined },
      { inFlight: undefined },
      { saved: undefined },
    ];
    const states: SyncState[] = [];
    for (const drop of drops) {
      states.push(summarize({ ...quiet, ...held }).state);
      held = { ...held, ...drop };
    }
    states.push(summarize({ ...quiet, ...held }).state);
    expect(states).toEqual(["conflict", "not-saved", "new-version", "offline", "reconnecting", "behind", "saving", "saved", "in-sync"]);
  });

  it("lets a conflict or a refused write outrank version skew, so a write skew stopped is red and never amber", () => {
    expect(summarize({ ...quiet, skew: true, problems: [problem("failed")] }).state).toBe("not-saved");
    expect(summarize({ ...quiet, skew: true, problems: [problem("conflict")] }).state).toBe("conflict");
    expect(summarize({ ...quiet, skew: true }).state).toBe("new-version");
  });

  it("carries each case's data", () => {
    const first = problem("rejected", "First");
    expect(summarize({ ...quiet, problems: [problem("conflict"), first, problem("failed", "Second")] })).toMatchObject({ state: "conflict", count: 1 });
    expect(summarize({ ...quiet, problems: [first, problem("failed", "Second")] })).toEqual({ state: "not-saved", count: 2, first });
    expect(summarize({ ...quiet, lag: lag(99_000, true) })).toEqual({ state: "behind", shown: 42, announced: 43 });
    expect(summarize({ ...quiet, lag: lag(quiet.now - SYNC_SHOW_AFTER_MS) })).toEqual({ state: "updating", shown: 42, announced: 43 });
    expect(summarize({ ...quiet, inFlight: { count: 3, since: 0 } })).toEqual({ state: "saving", count: 3 });
    expect(summarize({ ...quiet, saved: { at: quiet.now, time: "14:02:11" } })).toEqual({ state: "saved", time: "14:02:11" });
    expect(summarize({ ...quiet, stream: "idle" })).toEqual({ state: "in-sync", live: false, revision: 42 });
  });

  it("is quiet before a thing has taken long enough to matter", () => {
    expect(summarize({ ...quiet, inFlight: { count: 1, since: quiet.now - SYNC_SHOW_AFTER_MS + 1 } }).state).toBe("in-sync");
    expect(summarize({ ...quiet, inFlight: { count: 1, since: quiet.now - SYNC_SHOW_AFTER_MS } }).state).toBe("saving");
    expect(summarize({ ...quiet, lag: lag(quiet.now - SYNC_SHOW_AFTER_MS + 1) }).state).toBe("in-sync");
    expect(summarize({ ...quiet, lag: lag(quiet.now - SYNC_BEHIND_AFTER_MS + 1) }).state).toBe("updating");
    expect(summarize({ ...quiet, lag: lag(quiet.now - SYNC_BEHIND_AFTER_MS) }).state).toBe("behind");
    expect(summarize({ ...quiet, saved: { at: quiet.now - SYNC_SAVED_MS, time: "14:02:11" } }).state).toBe("in-sync");
  });
});

describe("syncWords", () => {
  it("colours each state by what it asks of the person", () => {
    const first = problem("rejected");
    const cases: [Parameters<typeof syncWords>[0], SyncTone][] = [
      [{ state: "conflict", count: 1, first }, "act"],
      [{ state: "not-saved", count: 1, first }, "act"],
      [{ state: "new-version" }, "wait"],
      [{ state: "offline" }, "wait"],
      [{ state: "reconnecting" }, "wait"],
      [{ state: "behind", shown: 1, announced: 2 }, "wait"],
      [{ state: "saving", count: 1 }, "working"],
      [{ state: "updating", shown: 1, announced: 2 }, "working"],
      [{ state: "saved", time: "14:02:11" }, "good"],
      [{ state: "in-sync", live: true, revision: 1 }, "good"],
    ];
    expect(cases.map(([shown]) => syncWords(shown).tone)).toEqual(cases.map(([, tone]) => tone));
    expect(cases.map(([shown]) => syncWords(shown).state)).toEqual(cases.map(([shown]) => shown.state));
  });

  it("announces what asks for action assertively and what waits politely", () => {
    const first = problem("rejected");
    expect(syncWords({ state: "conflict", count: 1, first }).announce).toBe("assertive");
    expect(syncWords({ state: "not-saved", count: 1, first }).announce).toBe("assertive");
    expect(syncWords({ state: "offline" }).announce).toBe("polite");
    expect(syncWords({ state: "behind", shown: 1, announced: 2 }).announce).toBe("polite");
    expect(syncWords({ state: "saving", count: 1 }).announce).toBeUndefined();
    expect(syncWords({ state: "in-sync", live: true, revision: 1 }).announce).toBeUndefined();
  });

  it("counts the refused writes, and names the first one's subject", () => {
    const shown = syncWords(summarize({ ...quiet, problems: [problem("rejected", "Draft test plan"), problem("failed", "Other")] }));
    expect(shown.label).toBe("NOT SAVED · 2");
    expect(shown.sentence).toContain("Draft test plan");
  });
});

describe("what the chip reads of its owners", () => {
  const timing: SyncTiming = { now: 1000, lag: undefined, inFlightSince: undefined, saved: undefined };
  const read: SyncReading = { stream: "live", online: true, skew: false, revision: 7, count: 0, problems: [], latest: undefined };

  it("makes the in-flight fact from the edit manager's count and the scheduler's stamp together", () => {
    expect(syncInputOf(timing, { ...read, count: 2 }).inFlight).toBeUndefined();
    expect(syncInputOf({ ...timing, inFlightSince: 500 }, read).inFlight).toBeUndefined();
    expect(syncInputOf({ ...timing, inFlightSince: 500 }, { ...read, count: 2 }).inFlight).toEqual({ count: 2, since: 500 });
  });

  it("says the wall time of the latest save, and nothing of a save until one is held", () => {
    expect(syncInputOf({ ...timing, saved: 900 }, read).saved).toBeUndefined();
    expect(syncInputOf({ ...timing, saved: 900 }, { ...read, latest: save(1) }).saved).toMatchObject({ at: 900 });
    expect(syncInputOf(timing, { ...read, latest: save(1) }).saved).toBeUndefined();
  });

  it("finds the earliest deadline still to come, and none once all passed or with nothing timed", () => {
    expect(nextDeadline(timing)).toBeUndefined();
    const timed: SyncTiming = { now: 1000, lag: lag(800), inFlightSince: 900, saved: 700 };
    // The lag shows at 1100 and is behind at 5800, the writes in flight show at 1200, SAVED ends at 2700.
    expect(nextDeadline(timed)).toBe(1100);
    expect(nextDeadline({ ...timed, now: 1100 })).toBe(1200);
    expect(nextDeadline({ ...timed, now: 5800 })).toBeUndefined();
  });

  it("announces a save for its SAVED time and each save separately, with its warning", () => {
    const saved: SyncTiming = { now: 1000, lag: undefined, inFlightSince: undefined, saved: 1000 };
    expect(savedAnnouncement(save(2, "Kickoff is now overdue."), saved)).toEqual({ id: 2, text: "Saved. Kickoff is now overdue." });
    expect(savedAnnouncement(save(3), saved)).toEqual({ id: 3, text: "Saved" });
    expect(savedAnnouncement(save(3), { ...saved, now: 1000 + SYNC_SAVED_MS })).toBeUndefined();
    expect(savedAnnouncement(undefined, saved)).toBeUndefined();
    expect(savedAnnouncement(save(3), { ...saved, saved: undefined })).toBeUndefined();
  });
});

/** A running scheduler on a simulated clock, and the chip's state as the session reads it. */
function scheduler() {
  const clock = new CountingClock();
  const actor = createActor(syncMachine, { clock, input: { monotonic: () => clock.now() } }).start();
  const read: SyncReading = { stream: "live", online: true, skew: false, revision: 7, count: 0, problems: [], latest: save(1) };
  return {
    clock,
    actor,
    send: (event: SyncEvent) => {
      actor.send(event);
    },
    advance: (ms: number) => {
      clock.increment(ms);
    },
    /** What the chip says now, with `count` writes in flight in the edit manager. */
    state: (count = 0): SyncState => syncSummary(syncInputOf(actor.getSnapshot().context.timing, { ...read, count })).state,
  };
}

describe("the scheduler wakes at each threshold with no event sent", () => {
  it("shows SAVING once a write has been out for 300ms and not a millisecond before", () => {
    const { send, advance, state } = scheduler();
    advance(1000);
    send({ type: "inFlight", count: 1 });
    advance(SYNC_SHOW_AFTER_MS - 1);
    expect(state(1)).toBe("in-sync");
    advance(1);
    expect(state(1)).toBe("saving");
  });

  it("shows UPDATING at 300ms of lag and BEHIND at 5s, and clears with the lag", () => {
    const { send, advance, state } = scheduler();
    advance(1000);
    send({ type: "lag", lag: lag(1000) });
    advance(SYNC_SHOW_AFTER_MS - 1);
    expect(state()).toBe("in-sync");
    advance(1);
    expect(state()).toBe("updating");
    advance(SYNC_BEHIND_AFTER_MS - SYNC_SHOW_AFTER_MS - 1);
    expect(state()).toBe("updating");
    advance(1);
    expect(state()).toBe("behind");
    send({ type: "lag", lag: undefined });
    expect(state()).toBe("in-sync");
  });

  it("shows SAVED for 2s after a save, and a second save restarts the time", () => {
    const { send, advance, state } = scheduler();
    advance(1000);
    send({ type: "saved" });
    expect(state()).toBe("saved");
    advance(1500);
    send({ type: "saved" });
    advance(SYNC_SAVED_MS - 1);
    expect(state()).toBe("saved");
    advance(1);
    expect(state()).toBe("in-sync");
  });

});

describe("the scheduler's writes in flight and its quiet", () => {
  it("keeps the start of the writes in flight while there are some, and starts it afresh after none", () => {
    const { send, advance, state } = scheduler();
    advance(1000);
    send({ type: "inFlight", count: 1 });
    advance(200);
    send({ type: "inFlight", count: 2 });
    advance(100);
    expect(state(2)).toBe("saving");
    send({ type: "inFlight", count: 0 });
    expect(state(0)).toBe("in-sync");
    advance(700);
    send({ type: "inFlight", count: 1 });
    advance(SYNC_SHOW_AFTER_MS - 1);
    expect(state(1)).toBe("in-sync");
    advance(1);
    expect(state(1)).toBe("saving");
  });

  it("takes no step for an event that changes nothing it holds", () => {
    const { actor, send, advance } = scheduler();
    advance(1000);
    send({ type: "inFlight", count: 1 });
    send({ type: "lag", lag: lag(900) });
    const before = actor.getSnapshot().context.timing;
    advance(10);
    send({ type: "inFlight", count: 4 });
    send({ type: "lag", lag: lag(900) });
    expect(actor.getSnapshot().context.timing).toBe(before);
  });

  it("waits for nothing when nothing is due, and leaves no timer once stopped", () => {
    const { clock, actor, send, advance } = scheduler();
    expect(clock.pending).toEqual([]);
    send({ type: "saved" });
    expect(clock.pending).toEqual([SYNC_SAVED_MS]);
    advance(SYNC_SAVED_MS);
    expect(clock.pending).toEqual([]);
    send({ type: "lag", lag: lag(clock.now()) });
    expect(clock.pending).not.toEqual([]);
    actor.stop();
    expect(clock.pending).toEqual([]);
  });
});

describe("the settled fact", () => {
  it.each([
    ["nothing under way, the stream live", { working: 0, inFlight: 0, stream: "live" }, true],
    ["nothing under way, nothing watched", { working: 0, inFlight: 0, stream: "idle" }, true],
    ["a read or a screen at work", { working: 1, inFlight: 0, stream: "live" }, false],
    ["a write in flight", { working: 0, inFlight: 1, stream: "live" }, false],
    ["the stream opening", { working: 0, inFlight: 0, stream: "connecting" }, false],
    ["the stream opening again", { working: 0, inFlight: 0, stream: "reconnecting" }, false],
  ] as const)("with %s", (_case, underway, expected) => {
    expect(settled(underway)).toBe(expected);
  });
});
