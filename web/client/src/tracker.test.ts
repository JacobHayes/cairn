// H6: revision tracking, case for case with the Rust client's
// (crates/api/src/client/tracker.rs), the failed-refetch fix included.
import { describe, expect, it } from "vitest";

import { UNTRACKED, fetched, forget, heldOf, refetchFailed, reopened, ticked, watchName, type RevisionOf, type Tick, type Tracked } from "./tracker.ts";

const deployment: RevisionOf = { domain: "deployment" };
const tick = (revision: number, of: RevisionOf = deployment): Tick => ({ of, revision });

/** What a view tracks after `received` asked it to refetch, or the same when it asked nothing. */
const hear = (tracked: Tracked, received: Tick): { tracked: Tracked; asks: boolean } => {
  const next = ticked(tracked, received);
  return { tracked: next ?? tracked, asks: next !== undefined };
};

describe("tracking", () => {
  it("asks for a refetch only for a newer tick, and only once", () => {
    let tracked = UNTRACKED;
    const heard = (revision: number): boolean => {
      const result = hear(tracked, tick(revision));
      tracked = result.tracked;
      return result.asks;
    };
    expect(heard(2)).toBe(true);
    expect(heard(2)).toBe(false);
    tracked = fetched(tracked, deployment, 3);
    expect(heard(2)).toBe(false);
    expect(heard(3)).toBe(false);
    expect(heard(4)).toBe(true);
    tracked = fetched(tracked, deployment, 2);
    expect(heldOf(tracked, deployment)).toBe(3);
    expect(heard(4)).toBe(false);
    tracked = fetched(tracked, deployment, 4);
    expect(heard(5)).toBe(true);
  });

  it("asks again after a failed refetch or a reopened stream", () => {
    const asked = hear(fetched(UNTRACKED, deployment, 4), tick(5));
    expect(asked.asks).toBe(true);
    const failed = refetchFailed(asked.tracked, deployment);
    expect(hear(failed, tick(5)).asks).toBe(true);
    expect(hear(reopened(asked.tracked), tick(5)).asks).toBe(true);
    expect(heldOf(reopened(asked.tracked), deployment)).toBe(4);
  });

  it("keeps each domain apart, and forgets one it no longer shows", () => {
    const journey: RevisionOf = { domain: { journey: "j_one" } };
    const tracked = fetched(fetched(UNTRACKED, journey, 7), deployment, 2);
    expect(hear(tracked, tick(7, journey)).asks).toBe(false);
    expect(hear(tracked, tick(3)).asks).toBe(true);
    const forgotten = forget(tracked, journey);
    expect(heldOf(forgotten, journey)).toBeUndefined();
    expect(heldOf(forgotten, deployment)).toBe(2);
    expect(hear(forgotten, tick(7, journey)).asks).toBe(true);
  });

  it("is a value: a function never changes the tracking it was given", () => {
    const before = fetched(UNTRACKED, deployment, 1);
    const snapshot = JSON.stringify(before);
    ticked(before, tick(2));
    fetched(before, deployment, 5);
    refetchFailed(before, deployment);
    forget(before, deployment);
    expect(JSON.stringify(before)).toBe(snapshot);
  });
});

describe("watchName", () => {
  it.each<[RevisionOf, string]>([
    [{ domain: "deployment" }, "deployment"],
    [{ domain: { journey: "j_one" } }, "journey:j_one"],
    [{ domain: { route: "r_one" } }, "route:r_one"],
    [{ proposal: "q_one" }, "proposal:q_one"],
  ])("names %j as the stream watches it", (of, name) => {
    expect(watchName(of)).toBe(name);
  });
});
