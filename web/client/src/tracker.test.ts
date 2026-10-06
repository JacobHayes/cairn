// H6: revision tracking, case for case with the Rust client's
// (crates/api/src/client/tracker.rs), the failed-refetch fix included.
import { describe, expect, it } from "vitest";

import { Tracker, watchName, type RevisionOf, type Tick } from "./tracker.ts";

const deployment: RevisionOf = { domain: "deployment" };
const tick = (revision: number, of: RevisionOf = deployment): Tick => ({ of, revision });

describe("Tracker", () => {
  it("asks for a refetch only for a newer tick, and only once", () => {
    const tracker = new Tracker();
    expect(tracker.ticked(tick(2))).toBe(true);
    expect(tracker.ticked(tick(2))).toBe(false);
    tracker.fetched(deployment, 3);
    expect(tracker.ticked(tick(2))).toBe(false);
    expect(tracker.ticked(tick(3))).toBe(false);
    expect(tracker.ticked(tick(4))).toBe(true);
    tracker.fetched(deployment, 2);
    expect(tracker.held(deployment)).toBe(3);
    expect(tracker.ticked(tick(4))).toBe(false);
    tracker.fetched(deployment, 4);
    expect(tracker.ticked(tick(5))).toBe(true);
  });

  it("asks again after a failed refetch or a reopened stream", () => {
    const tracker = new Tracker();
    tracker.fetched(deployment, 4);
    expect(tracker.ticked(tick(5))).toBe(true);
    tracker.refetchFailed(deployment);
    expect(tracker.ticked(tick(5))).toBe(true);
    tracker.reopened();
    expect(tracker.ticked(tick(5))).toBe(true);
    expect(tracker.held(deployment)).toBe(4);
  });

  it("keeps each domain apart, and forgets one it no longer shows", () => {
    const tracker = new Tracker();
    const journey: RevisionOf = { domain: { journey: "j_one" } };
    tracker.fetched(journey, 7);
    tracker.fetched(deployment, 2);
    expect(tracker.ticked(tick(7, journey))).toBe(false);
    expect(tracker.ticked(tick(3))).toBe(true);
    tracker.forget(journey);
    expect(tracker.held(journey)).toBeUndefined();
    expect(tracker.ticked(tick(7, journey))).toBe(true);
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
