// H6 for a derived journey: refetched only for a tick newer than what is held, for a newer
// deployment, and when the date passes; never shown older; skew stops it (ARCHITECTURE, Web
// UI: data flow, version skew).
import { describe, expect, it } from "vitest";

import { ENGINE, FakeDeriver, FakeHost, manualTimers, settled } from "./fake.test-support.ts";
import { PARKED_JOURNEY_COUNT_MAX } from "./journeys.ts";
import { Session } from "./session.ts";

const TODAY = "2026-10-06";

async function started(journeys: Record<string, number> = { j_one: 3 }) {
  const host = new FakeHost();
  for (const [id, revision] of Object.entries(journeys)) {
    host.journeysHeld.set(id, { revision, deployment: 1, today: TODAY, engine: ENGINE });
  }
  const deriver = new FakeDeriver();
  const { timers, pending } = manualTimers();
  let now = new Date(`${TODAY}T12:00:00Z`);
  const session = await Session.start({ host, deriver, timers, now: () => now });
  const setNow = (at: string) => {
    now = new Date(at);
  };
  return { host, deriver, session, pending, setNow };
}

function revisionShown(session: Session, id: string) {
  const view = session.journeys.view(id);
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
    const { host, session } = await started();
    session.journeys.mount("j_one");
    await settled();
    host.open();
    await settled();
    expect(revisionShown(session, "j_one")).toEqual([3, 1, TODAY]);
    edit(host, "j_one", 4);
    await settled();
    host.tick({ of: { domain: { journey: "j_one" } }, revision: 4 });
    host.tick({ of: { domain: { journey: "j_one" } }, revision: 2 });
    await settled();
    expect(revisionShown(session, "j_one")).toEqual([4, 1, TODAY]);
    expect(host.fetches.get("j_one")).toBe(2);
  });

  it("shown again after leaving it is refetched only if it moved meanwhile", async () => {
    const { host, session } = await started({ j_one: 3, j_two: 1 });
    const leave = session.journeys.mount("j_one");
    await settled();
    host.open();
    edit(host, "j_one", 4);
    await settled();
    leave();
    session.journeys.mount("j_two");
    await settled();
    host.open();
    await settled();
    session.journeys.mount("j_one");
    await settled();
    host.open();
    await settled();
    expect(revisionShown(session, "j_one")).toEqual([4, 1, TODAY]);
    expect(host.fetches.get("j_one")).toBe(2);
  });

  it("re-derives for a newer deployment, and not for the deployment it already has", async () => {
    const { host, session } = await started();
    session.journeys.mount("j_one");
    await settled();
    host.open();
    await settled();
    host.deploymentRevision = 2;
    host.journeysHeld.set("j_one", { revision: 3, deployment: 2, today: TODAY, engine: ENGINE });
    host.tick({ of: { domain: "deployment" }, revision: 2 });
    await settled();
    expect(revisionShown(session, "j_one")).toEqual([3, 2, TODAY]);
    expect(session.deployment.current?.revision).toBe(2);
    host.open();
    await settled();
    expect(host.fetches.get("j_one")).toBe(2);
    expect(session.deployment.fetches).toBe(2);
  });

});

describe("a shown journey over time", () => {
  it("is refetched once when the date passes in the deployment's zone", async () => {
    const { host, session, pending, setNow } = await started();
    session.journeys.mount("j_one");
    await settled();
    const check = () => {
      pending.shift()?.callback();
    };
    check();
    expect(host.fetches.get("j_one")).toBe(1);
    setNow("2026-10-07T00:00:30Z");
    host.journeysHeld.set("j_one", { revision: 3, deployment: 1, today: "2026-10-07", engine: ENGINE });
    check();
    check();
    await settled();
    expect(revisionShown(session, "j_one")).toEqual([3, 1, "2026-10-07"]);
    expect(host.fetches.get("j_one")).toBe(2);
  });
});

describe("a journey's faults", () => {
  it("from a newer engine latches skew, is not derived, and stops refetching", async () => {
    const { host, session } = await started();
    session.journeys.mount("j_one");
    await settled();
    host.journeysHeld.set("j_one", { revision: 4, deployment: 1, today: TODAY, engine: "2.0.0" });
    host.open();
    edit(host, "j_one", 4);
    await settled();
    expect(session.skew.current).toEqual({ document: "2.0.0", engine: ENGINE });
    expect(revisionShown(session, "j_one")).toEqual([3, 1, TODAY]);
    edit(host, "j_one", 5);
    await settled();
    expect(host.fetches.get("j_one")).toBe(2);
  });

  it("a failed refetch is tried again after the reopen delay", async () => {
    const { host, session, pending } = await started();
    session.journeys.mount("j_one");
    await settled();
    host.open();
    host.failNext = true;
    edit(host, "j_one", 4);
    await settled();
    expect(revisionShown(session, "j_one")).toEqual([3, 1, TODAY]);
    const retry = pending.find((timer) => timer.ms > 0 && timer.ms < 60_000);
    retry?.callback();
    await settled();
    expect(revisionShown(session, "j_one")).toEqual([4, 1, TODAY]);
  });

  it("deleted (listed at revision 0 by a stream's current revisions) shows missing", async () => {
    const { host, session } = await started();
    session.journeys.mount("j_one");
    await settled();
    host.open();
    await settled();
    host.journeysHeld.delete("j_one");
    host.open();
    await settled();
    expect(session.journeys.view("j_one").status).toBe("missing");
  });

  it("is released from the worker once too many journeys are parked", async () => {
    const ids = Array.from({ length: PARKED_JOURNEY_COUNT_MAX + 2 }, (_, at) => `j_${String(at)}`);
    const { deriver, session } = await started(Object.fromEntries(ids.map((id) => [id, 1])));
    for (const id of ids) {
      session.journeys.mount(id)();
    }
    await settled();
    expect(deriver.released).toEqual(ids.slice(0, 2));
  });
});
