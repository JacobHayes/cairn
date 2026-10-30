// H6 for a derived journey: refetched only for a tick newer than what is held, for a newer
// deployment, and when the date passes; never shown older; skew stops it; and a derivation is
// freed only by the load that made it (ARCHITECTURE, Web UI: data flow, version skew).
import { SSE_COALESCING_INTERVAL_MS } from "@cairn/client";
import { describe, expect, it } from "vitest";

import { CountingClock, FakeDeriver, FakeHost, deferred, settled } from "./fake.test-support.ts";
import { PARKED_JOURNEY_COUNT_MAX, journeyView, lagOf } from "./journeys.ts";
import { Session } from "./session.ts";
import { HOLDS_RETRY_MS, ROLLOVER_CHECK_MS } from "./today.ts";

const TODAY = "2026-10-06";

async function started(journeys: Record<string, number> = { j_one: 3 }) {
  const host = new FakeHost();
  for (const [id, revision] of Object.entries(journeys)) {
    host.journeysHeld.set(id, { revision, deployment: 1, today: TODAY });
  }
  const deriver = new FakeDeriver();
  const clock = new CountingClock();
  let now = new Date(`${TODAY}T12:00:00Z`);
  const session = await Session.start({ host, deriver, clock, now: () => now });
  const setNow = (at: string) => {
    now = new Date(at);
  };
  /** Lets what the machines raised for themselves run, and every pending promise. */
  const turn = async () => {
    clock.increment(0);
    await settled();
  };
  const mount = (id: string) => {
    session.journeys.send({ type: "mount", id });
    return () => {
      session.journeys.send({ type: "unmount", id });
    };
  };
  await turn();
  return { host, deriver, session, clock, setNow, turn, mount };
}

const viewOf = (session: Session, id: string) => journeyView(session.journeys.getSnapshot(), id);

function revisionShown(session: Session, id: string) {
  const view = viewOf(session, id);
  return view.status === "ready" ? [view.key.revision, view.key.deployment_revision, view.key.today] : view.status;
}

function edit(host: FakeHost, id: string, revision: number) {
  const held = host.journeysHeld.get(id);
  if (held !== undefined) {
    host.journeysHeld.set(id, { ...held, revision });
  }
  host.tick({ of: { domain: { journey: id } }, revision });
}

describe("a shown journey", () => {
  it("is derived once, and refetched only for a newer tick", async () => {
    const { host, session, mount, turn } = await started();
    mount("j_one");
    await turn();
    host.open();
    await turn();
    expect(revisionShown(session, "j_one")).toEqual([3, 1, TODAY]);
    edit(host, "j_one", 4);
    await turn();
    host.tick({ of: { domain: { journey: "j_one" } }, revision: 4 });
    host.tick({ of: { domain: { journey: "j_one" } }, revision: 2 });
    await turn();
    expect(revisionShown(session, "j_one")).toEqual([4, 1, TODAY]);
    expect(host.fetches.get("j_one")).toBe(2);
  });

  it("shown again after leaving it is refetched only if it moved meanwhile", async () => {
    const { host, session, mount, turn } = await started({ j_one: 3, j_two: 1 });
    const leave = mount("j_one");
    await turn();
    host.open();
    edit(host, "j_one", 4);
    await turn();
    leave();
    mount("j_two");
    await turn();
    host.open();
    await turn();
    mount("j_one");
    await turn();
    host.open();
    await turn();
    expect(revisionShown(session, "j_one")).toEqual([4, 1, TODAY]);
    expect(host.fetches.get("j_one")).toBe(2);
  });

});

describe("a journey shown again", () => {
  it("shown again after leaving it is refetched if it moved while it was not watched", async () => {
    const { host, session, mount, turn } = await started({ j_one: 3, j_two: 1 });
    const leave = mount("j_one");
    await turn();
    host.open();
    await turn();
    leave();
    mount("j_two");
    await turn();
    edit(host, "j_one", 5);
    await turn();
    expect(host.fetches.get("j_one")).toBe(1);
    mount("j_one");
    await turn();
    host.open();
    await turn();
    expect(revisionShown(session, "j_one")).toEqual([5, 1, TODAY]);
  });

  it("re-derives for a newer deployment, and not for the deployment it already has", async () => {
    const { host, session, mount, turn } = await started();
    mount("j_one");
    await turn();
    host.open();
    await turn();
    host.deploymentRevision = 2;
    host.journeysHeld.set("j_one", { revision: 3, deployment: 2, today: TODAY });
    host.tick({ of: { domain: "deployment" }, revision: 2 });
    await turn();
    expect(revisionShown(session, "j_one")).toEqual([3, 2, TODAY]);
    expect(session.deployment.getSnapshot().context.held?.value).toMatchObject({ revision: 2 });
    host.open();
    await turn();
    expect(host.fetches.get("j_one")).toBe(2);
    expect(host.deploymentFetches).toBe(2);
  });
});

describe("a shown journey over time", () => {
  it("is refetched once when the date passes in the deployment's zone", async () => {
    const { host, session, mount, turn, clock, setNow } = await started();
    mount("j_one");
    await turn();
    clock.increment(ROLLOVER_CHECK_MS);
    await turn();
    expect(host.fetches.get("j_one")).toBe(1);
    setNow("2026-10-07T00:00:30Z");
    host.journeysHeld.set("j_one", { revision: 3, deployment: 1, today: "2026-10-07" });
    clock.increment(ROLLOVER_CHECK_MS);
    await turn();
    clock.increment(ROLLOVER_CHECK_MS);
    await turn();
    expect(revisionShown(session, "j_one")).toEqual([3, 1, "2026-10-07"]);
    expect(host.fetches.get("j_one")).toBe(2);
  });

  it("keeps asking at each rollover check until the host's today catches up", async () => {
    const { host, session, mount, turn, clock, setNow } = await started();
    mount("j_one");
    await turn();
    const check = async () => {
      clock.increment(ROLLOVER_CHECK_MS);
      await turn();
    };
    await check();
    setNow("2026-10-07T00:00:10Z");
    await check();
    expect(revisionShown(session, "j_one")).toEqual([3, 1, TODAY]);
    host.journeysHeld.set("j_one", { revision: 3, deployment: 1, today: "2026-10-07" });
    setNow("2026-10-07T00:01:10Z");
    await check();
    expect(revisionShown(session, "j_one")).toEqual([3, 1, "2026-10-07"]);
    const fetched = host.fetches.get("j_one");
    await check();
    expect(host.fetches.get("j_one")).toBe(fetched);
  });
});

describe("a journey's faults", () => {
  it("from another engine latches skew, is not derived, and stops refetching", async () => {
    const { host, session, mount, turn } = await started();
    mount("j_one");
    await turn();
    host.journeysHeld.set("j_one", { revision: 4, deployment: 1, today: TODAY, skewed: true });
    host.open();
    edit(host, "j_one", 4);
    await turn();
    expect(session.skewed).toBe(true);
    expect(revisionShown(session, "j_one")).toEqual([3, 1, TODAY]);
    edit(host, "j_one", 5);
    await turn();
    expect(host.fetches.get("j_one")).toBe(2);
  });

  it("a failed refetch is tried again after the reopen delay", async () => {
    const { host, session, mount, turn, clock } = await started();
    mount("j_one");
    await turn();
    host.open();
    host.failNext = true;
    edit(host, "j_one", 4);
    await turn();
    expect(revisionShown(session, "j_one")).toEqual([3, 1, TODAY]);
    clock.increment(SSE_COALESCING_INTERVAL_MS);
    await turn();
    expect(revisionShown(session, "j_one")).toEqual([4, 1, TODAY]);
  });

  it("is asked again at once by a click on BEHIND, without waiting out the delay", async () => {
    const { host, session, mount, turn } = await started();
    mount("j_one");
    await turn();
    host.open();
    host.failNext = true;
    edit(host, "j_one", 4);
    await turn();
    expect(revisionShown(session, "j_one")).toEqual([3, 1, TODAY]);
    session.root.send({ type: "reopen" });
    await turn();
    host.open();
    await turn();
    expect(revisionShown(session, "j_one")).toEqual([4, 1, TODAY]);
  });

});

describe("a journey that cannot be read", () => {
  it("deleted (listed at revision 0 by a stream's current revisions) shows missing", async () => {
    const { host, session, mount, turn } = await started();
    mount("j_one");
    await turn();
    host.open();
    await turn();
    host.journeysHeld.delete("j_one");
    host.open();
    await turn();
    expect(viewOf(session, "j_one").status).toBe("missing");
  });

  it("is released from the worker once too many journeys are parked", async () => {
    const ids = Array.from({ length: PARKED_JOURNEY_COUNT_MAX + 2 }, (_, at) => `j_${String(at)}`);
    const { deriver, mount, turn } = await started(Object.fromEntries(ids.map((id) => [id, 1])));
    for (const id of ids) {
      const leave = mount(id);
      await turn();
      leave();
    }
    await turn();
    expect(deriver.released.map((handle) => handle.journey)).toEqual(ids.slice(0, 2));
  });
});

describe("the first fetch of a journey", () => {
  it("re-derives a journey whose first fetch read the document before a deployment tick", async () => {
    const { host, session, mount, turn } = await started({ j_one: 3, j_two: 3 });
    mount("j_two");
    await turn();
    host.open();
    await turn();
    mount("j_one");
    host.deploymentRevision = 2;
    for (const id of ["j_one", "j_two"]) {
      host.journeysHeld.set(id, { revision: 3, deployment: 2, today: TODAY });
    }
    host.tick({ of: { domain: "deployment" }, revision: 2 });
    await turn();
    expect(revisionShown(session, "j_two")).toEqual([3, 2, TODAY]);
    expect(revisionShown(session, "j_one")).toEqual([3, 2, TODAY]);
  });

  it("reads again when a deployment tick arrives during the fetch and the document is older than it", async () => {
    const { host, deriver, session, mount, turn } = await started();
    const first = deferred<undefined>();
    const second = deferred<undefined>();
    host.holdDocument.set("j_one", first.promise);
    mount("j_one");
    await turn();
    host.open();
    host.tick({ of: { domain: "deployment" }, revision: 2 });
    host.holdDocument.set("j_one", second.promise);
    first.resolve(undefined);
    await turn();
    // The first document was read at deployment 1, which the tick has passed: it is read again.
    expect(host.fetches.get("j_one")).toBe(2);
    host.deploymentRevision = 2;
    host.journeysHeld.set("j_one", { revision: 3, deployment: 2, today: TODAY });
    second.resolve(undefined);
    await turn();
    expect(revisionShown(session, "j_one")).toEqual([3, 2, TODAY]);
    expect(deriver.loaded).toHaveLength(2);
  });

  it("reads once when the document the fetch returns already has the deployment the tick named", async () => {
    const { host, deriver, session, mount, turn } = await started();
    const gate = deferred<undefined>();
    host.holdDocument.set("j_one", gate.promise);
    mount("j_one");
    await turn();
    host.open();
    host.tick({ of: { domain: "deployment" }, revision: 2 });
    host.deploymentRevision = 2;
    host.journeysHeld.set("j_one", { revision: 3, deployment: 2, today: TODAY });
    gate.resolve(undefined);
    await turn();
    expect(revisionShown(session, "j_one")).toEqual([3, 2, TODAY]);
    expect(host.fetches.get("j_one")).toBe(1);
    expect(deriver.loaded).toHaveLength(1);
  });
});

describe("a journey deleted during a derive", () => {
  it("shows missing, frees what the derive made, and is not tried again", async () => {
    const { host, deriver, session, mount, turn, clock } = await started();
    const gate = deferred<undefined>();
    deriver.holdLoad = gate.promise;
    mount("j_one");
    await turn();
    host.open();
    host.journeysHeld.delete("j_one");
    host.tick({ of: { domain: { journey: "j_one" } }, revision: 0 });
    await turn();
    expect(viewOf(session, "j_one").status).toBe("missing");
    gate.resolve(undefined);
    await turn();
    expect(deriver.held.has("j_one")).toBe(false);
    expect(viewOf(session, "j_one").status).toBe("missing");
    const fetched = host.fetches.get("j_one");
    clock.increment(60_000);
    await turn();
    expect(host.fetches.get("j_one")).toBe(fetched);
  });
});

describe("a derivation that holds until a time of day", () => {
  it("is refetched at that moment, not at the next rollover check", async () => {
    const { host, deriver, mount, turn, clock } = await started();
    deriver.holdsUntil = `${TODAY}T12:00:20Z`;
    mount("j_one");
    await turn();
    expect(host.fetches.get("j_one")).toBe(1);
    clock.increment(19_999);
    await turn();
    expect(host.fetches.get("j_one")).toBe(1);
    clock.increment(1);
    await turn();
    expect(host.fetches.get("j_one")).toBe(2);
  });
});

describe("a derivation whose refetch at its moment failed", () => {
  it("is not asked for again on the moment's own timer, only on the read's retry terms", async () => {
    const { host, deriver, mount, turn, clock } = await started();
    deriver.holdsUntil = `${TODAY}T12:00:20Z`;
    mount("j_one");
    await turn();
    host.failNext = true;
    clock.increment(20_000);
    await turn();
    expect(host.fetches.get("j_one")).toBe(2);
    expect(clock.pending).not.toContain(HOLDS_RETRY_MS);
  });
});

describe("a retry that is no longer owed", () => {
  it("is gone after a failure and then a success: only the rollover check is left to wait for", async () => {
    const { host, mount, turn, clock } = await started();
    mount("j_one");
    await turn();
    host.open();
    host.failNext = true;
    edit(host, "j_one", 4);
    await turn();
    expect(clock.pending).toContain(SSE_COALESCING_INTERVAL_MS);
    host.open();
    await turn();
    expect(clock.pending).toEqual([ROLLOVER_CHECK_MS]);
  });

  it("is gone after a failure and then the journey's deletion, and nothing reads the gone journey later", async () => {
    const { host, mount, turn, clock } = await started();
    mount("j_one");
    await turn();
    host.open();
    host.failNext = true;
    edit(host, "j_one", 4);
    await turn();
    host.journeysHeld.delete("j_one");
    host.tick({ of: { domain: { journey: "j_one" } }, revision: 0 });
    await turn();
    expect(clock.pending).toEqual([ROLLOVER_CHECK_MS]);
    const fetched = host.fetches.get("j_one");
    clock.increment(60_000);
    await turn();
    expect(host.fetches.get("j_one")).toBe(fetched);
  });
});

describe("an owner that was evicted", () => {
  /** Mounts and leaves enough other journeys that `j_one`, the least recently shown, is released. */
  async function evicted() {
    const others = Array.from({ length: PARKED_JOURNEY_COUNT_MAX }, (_, at) => `j_other${String(at)}`);
    const state = await started(Object.fromEntries(["j_one", ...others].map((id) => [id, 1])));
    return { ...state, others };
  }

  it("cannot free the derivation of the owner that replaced it, whenever its load answers", async () => {
    const { deriver, session, mount, turn, others } = await evicted();
    const gate = deferred<undefined>();
    deriver.holdLoad = gate.promise;
    const leave = mount("j_one");
    await turn();
    leave();
    for (const id of others) {
      mount(id)();
    }
    await turn();
    expect(Object.keys(session.journeys.getSnapshot().context.entries)).not.toContain("j_one");
    mount("j_one");
    await turn();
    expect(revisionShown(session, "j_one")).toEqual([1, 1, TODAY]);
    const current = deriver.held.get("j_one");
    gate.resolve(undefined);
    await turn();
    // The evicted owner's load was replaced; whatever it frees, the successor's derivation stays.
    expect(deriver.held.get("j_one")).toBe(current);
    await expect(deriver.derived("j_one")).resolves.toBeDefined();
  });

  it("posts no load for a document that arrives after it was evicted", async () => {
    const { host, deriver, session, mount, turn, others } = await evicted();
    const gate = deferred<undefined>();
    host.holdDocument.set("j_one", gate.promise);
    const leave = mount("j_one");
    await turn();
    leave();
    for (const id of others) {
      mount(id)();
    }
    await turn();
    mount("j_one");
    await turn();
    expect(revisionShown(session, "j_one")).toEqual([1, 1, TODAY]);
    const loads = deriver.loaded.filter((handle) => handle.journey === "j_one");
    gate.resolve(undefined);
    await turn();
    expect(deriver.loaded.filter((handle) => handle.journey === "j_one")).toEqual(loads);
    expect(deriver.held.get("j_one")).toBe(loads.at(-1)?.load);
  });
});

describe("version skew while a fetch is out", () => {
  it("posts no load for the document, and keeps the view as it was", async () => {
    const { host, deriver, session, mount, turn } = await started({ j_one: 3, j_two: 1 });
    mount("j_one");
    mount("j_two");
    await turn();
    host.open();
    await turn();
    const gate = deferred<undefined>();
    host.holdDocument.set("j_one", gate.promise);
    edit(host, "j_one", 4);
    await turn();
    host.journeysHeld.set("j_two", { revision: 2, deployment: 1, today: TODAY, skewed: true });
    edit(host, "j_two", 2);
    await turn();
    expect(session.skewed).toBe(true);
    const loads = deriver.loaded.length;
    gate.resolve(undefined);
    await turn();
    expect(deriver.loaded).toHaveLength(loads);
    expect(revisionShown(session, "j_one")).toEqual([3, 1, TODAY]);
  });
});

describe("shutting the session down", () => {
  it("leaves no derivation, no stream, no timer and no request behind", async () => {
    const { host, deriver, session, mount, turn, clock } = await started({ j_one: 3, j_two: 1 });
    mount("j_one");
    mount("j_two")();
    await turn();
    host.open();
    host.failNext = true;
    edit(host, "j_one", 4);
    await turn();
    session.close();
    expect(deriver.held.size).toBe(0);
    const freed = deriver.released.map((handle) => `${handle.journey}:${String(handle.load)}`);
    expect(new Set(freed).size).toBe(freed.length);
    expect(host.streams.every((stream) => stream.closed)).toBe(true);
    expect(clock.pending).toEqual([]);
    const fetched = [...host.fetches.values()];
    clock.increment(60_000);
    await turn();
    expect([...host.fetches.values()]).toEqual(fetched);
  });
});

describe("the journeys behind", () => {
  it("reports the failed refetch among journeys behind, not the one that has waited longest", async () => {
    const { host, session, mount, turn } = await started({ j_one: 3, j_two: 3 });
    mount("j_one");
    mount("j_two");
    await turn();
    host.open();
    await turn();
    let release = () => {};
    host.holdNext = new Promise((resolve) => {
      release = resolve;
    });
    edit(host, "j_one", 4);
    await turn();
    host.failNext = true;
    edit(host, "j_two", 4);
    await turn();
    expect(lagOf(session.journeys.getSnapshot())).toMatchObject({ subject: "journey:j_two", failed: true });
    release();
  });
});
