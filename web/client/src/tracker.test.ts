// H6: revision tracking. The cases are tracker-cases.json, which the Rust client's tracker
// (crates/api/src/client/tracker.rs) runs too, so the two cannot drift; `forget` is this
// client's alone.
import { describe, expect, it } from "vitest";

import cases from "./tracker-cases.json" with { type: "json" };
import { UNTRACKED, fetched, forget, heldOf, refetchFailed, reopened, ticked, watchName, type RevisionOf, type Tick, type Tracked } from "./tracker.ts";

const deployment: RevisionOf = { domain: "deployment" };
const tick = (revision: number, of: RevisionOf = deployment): Tick => ({ of, revision });

/** What a view tracks after `received` asked it to refetch, or the same when it asked nothing. */
const hear = (tracked: Tracked, received: Tick): { tracked: Tracked; asks: boolean } => {
  const next = ticked(tracked, received);
  return { tracked: next ?? tracked, asks: next !== undefined };
};

/** One step of a shared case: the move, where it applies (the deployment unless named), and what it must show. */
interface Step {
  op: "fetched" | "ticked" | "refetch_failed" | "reopened" | "held";
  of?: RevisionOf;
  revision?: number;
  asks?: boolean;
  holds?: number | null;
}

describe("tracking", () => {
  it.each(cases as { name: string; steps: Step[] }[])("$name", ({ steps }) => {
    let tracked = UNTRACKED;
    for (const [index, step] of steps.entries()) {
      const of = step.of ?? deployment;
      const at = `step ${String(index)} (${step.op})`;
      switch (step.op) {
        case "fetched":
          tracked = fetched(tracked, of, step.revision ?? 0);
          break;
        case "ticked": {
          const result = hear(tracked, tick(step.revision ?? 0, of));
          tracked = result.tracked;
          expect(result.asks, at).toBe(step.asks);
          break;
        }
        case "refetch_failed":
          tracked = refetchFailed(tracked, of);
          break;
        case "reopened":
          tracked = reopened(tracked);
          break;
        case "held":
          expect(heldOf(tracked, of) ?? null, at).toBe(step.holds);
          break;
      }
    }
  });

  it("forgets a domain it no longer shows", () => {
    const journey: RevisionOf = { domain: { journey: "j_one" } };
    const tracked = fetched(fetched(UNTRACKED, journey, 7), deployment, 2);
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
