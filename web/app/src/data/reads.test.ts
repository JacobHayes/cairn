// H6 for the journey index: "upgrade available" moves when a route publishes, with no journey
// tick, so a route tick refetches the index; a journey outside the filter asks once per
// revision, since a newer one may bring it in.
import { describe, expect, it } from "vitest";

import { createActor } from "xstate";

import { CountingClock, FakeDeriver, FakeHost, settled } from "./fake.test-support.ts";
import type { JourneyIndexQuery } from "./host.ts";
import { keyedRead, valueOf, readView } from "./keyed-read.ts";
import { liveSource } from "./live.ts";
import { indexKey, journeyIndex, type JourneySummary } from "./reads.ts";
import { Session } from "./session.ts";

/** A host with one journey on version 1 of route `r`, whose latest version the test sets. */
class RouteHost extends FakeHost {
  latest = 1;
  override journeys(query?: JourneyIndexQuery) {
    this.queries.push(query);
    const item: JourneySummary = {
      id: "j_one",
      name: "One",
      status: "active",
      revision: 3,
      created_at: "2026-10-01T00:00:00Z",
      lineage: { route: "r", version: 1 },
      latest_version: this.latest,
      upgrade_available: this.latest > 1,
    };
    return Promise.resolve({ items: [item] });
  }
}

async function reading(query: JourneyIndexQuery) {
  const host = new RouteHost();
  const clock = new CountingClock();
  const session = await Session.start({ host, deriver: new FakeDeriver(), clock });
  const read = createActor(keyedRead, { clock, input: { source: liveSource<JourneySummary[]>(), env: session.readEnv } }).start();
  read.send({ type: "want", key: { identity: [indexKey(query)], freshness: [], args: { spec: journeyIndex(query)(session) } } });
  clock.increment(0);
  await settled();
  host.open();
  await settled();
  const view = () => readView<JourneySummary[]>(read.getSnapshot(), [indexKey(query)], []);
  return { host, view };
}

describe("the journey index kept current", () => {
  it("refetches when a journey's route publishes a newer version", async () => {
    const { host, view } = await reading({ status: ["active"] });
    expect(valueOf(view())).toMatchObject([{ upgrade_available: false }]);
    host.latest = 2;
    host.tick({ of: { domain: { route: "r" } }, revision: 9 });
    await settled();
    expect(valueOf(view())).toMatchObject([{ upgrade_available: true }]);
  });

  it("asks once per revision of a journey outside its filter", async () => {
    const { host } = await reading({ route: "r", version: 1 });
    const before = host.queries.length;
    for (const revision of [1, 1, 2, 2]) {
      host.tick({ of: { domain: { journey: "j_elsewhere" } }, revision });
      await settled();
    }
    expect(host.queries.length - before).toBe(2);
  });
});
