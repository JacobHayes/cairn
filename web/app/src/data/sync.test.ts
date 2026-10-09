// What the sync chip says (design 9): the precedence among its states, and the time thresholds
// that keep it from flickering.
import { describe, expect, it } from "vitest";

import type { Lag } from "./journeys.ts";
import { SYNC_BEHIND_AFTER_MS, SYNC_SAVED_MS, SYNC_SHOW_AFTER_MS, SyncStatus, summarize, type Problem, type SyncInput, type SyncState } from "./sync.ts";

const quiet: SyncInput = {
  now: 100_000,
  stream: "live",
  online: true,
  demo: false,
  skew: undefined,
  problems: [],
  inFlight: 0,
  inFlightSince: undefined,
  lag: undefined,
  saved: undefined,
  revision: 42,
};

const problem = (kind: Problem["kind"]): Problem => ({ kind, message: "no", label: "Draft test plan", address: undefined, discard: () => undefined });
const lag = (since: number, failed = false): Lag => ({ journey: "j_one", shown: 42, announced: 43, since, failed });

describe("summarize", () => {
  it("names the revision a view is behind, and the one it shows", () => {
    const summary = summarize({ ...quiet, lag: lag(99_000, true) });
    expect(summary.label).toBe("BEHIND · REV 43");
  });

  it("puts the first state of the precedence on top of the others that hold", () => {
    let held: Partial<SyncInput> = {
      problems: [problem("conflict"), problem("rejected")],
      skew: { document: "2.0.0", engine: "1.0.0" },
      online: false,
      stream: "reconnecting",
      lag: lag(90_000),
      inFlight: 1,
      inFlightSince: 90_000,
      saved: { at: 99_500, time: "14:02:11" },
      demo: true,
    };
    // Take away what holds, top of the precedence first: the next state shows.
    const drops: Partial<SyncInput>[] = [
      { problems: [problem("rejected")] },
      { problems: [] },
      { skew: undefined },
      { online: true },
      { stream: "live" },
      { lag: undefined },
      { inFlight: 0 },
      { saved: undefined },
    ];
    const states: SyncState[] = [];
    for (const drop of drops) {
      states.push(summarize({ ...quiet, ...held }).state);
      held = { ...held, ...drop };
    }
    states.push(summarize({ ...quiet, ...held }).state);
    expect(states).toEqual(["conflict", "not-saved", "new-version", "offline", "reconnecting", "behind", "saving", "saved", "demo"]);
  });

  it("counts every rejection that is not a conflict as not saved", () => {
    expect(summarize({ ...quiet, problems: [problem("rejected"), problem("failed")] }).label).toBe("NOT SAVED · 2");
  });

  it("says a view behind only on its deployment is behind, without naming a revision", () => {
    expect(summarize({ ...quiet, lag: { ...lag(99_000, true), announced: 42 } })).toMatchObject({ state: "behind", label: "BEHIND" });
  });

  it("is quiet before a thing has taken long enough to matter", () => {
    expect(summarize({ ...quiet, inFlight: 1, inFlightSince: quiet.now - SYNC_SHOW_AFTER_MS + 1 }).state).toBe("in-sync");
    expect(summarize({ ...quiet, lag: lag(quiet.now - SYNC_SHOW_AFTER_MS + 1) }).state).toBe("in-sync");
    expect(summarize({ ...quiet, lag: lag(quiet.now - SYNC_BEHIND_AFTER_MS + 1) }).state).toBe("updating");
    expect(summarize({ ...quiet, lag: lag(quiet.now - SYNC_BEHIND_AFTER_MS) }).state).toBe("behind");
    expect(summarize({ ...quiet, saved: { at: quiet.now - SYNC_SAVED_MS, time: "14:02:11" } }).state).toBe("in-sync");
  });
});

function setup() {
  let now = 0;
  const timers = new Map<number, { at: number; callback: () => void }>();
  let next = 0;
  const status = new SyncStatus(
    {
      set: (callback, ms) => {
        timers.set(++next, { at: now + ms, callback });
        return next;
      },
      clear: (handle) => {
        timers.delete(handle as number);
      },
    },
    () => now,
    false,
  );
  const advance = (ms: number) => {
    now += ms;
    for (const [handle, timer] of [...timers]) {
      if (timer.at <= now) {
        timers.delete(handle);
        timer.callback();
      }
    }
  };
  return { status, advance };
}

describe("SyncStatus", () => {
  it("shows SAVING only for a write still out after 300ms, then SAVED for 2s", async () => {
    const { status, advance } = setup();
    let finish: () => void = () => undefined;
    const sending = status.track(new Promise<void>((resolve) => { finish = resolve; }));
    expect(status.summary.state).toBe("in-sync");
    advance(SYNC_SHOW_AFTER_MS);
    expect(status.summary.state).toBe("saving");
    finish();
    await sending;
    status.saved("14:02:11");
    expect(status.summary.state).toBe("saved");
    advance(SYNC_SAVED_MS);
    expect(status.summary.state).toBe("in-sync");
  });
});
