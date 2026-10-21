// A route canvas's reads (ARCHITECTURE, Web UI: reads): what the route graph read finds for a
// draft, a version and a route with nothing to draw, which ticks keep it current and what it
// holds, and how a shown graph is derived and held.
import { describe, expect, it } from "vitest";

import type { Deployment, Deriver, Route, RouteVersion } from "../data/host.ts";
import { sameKey, type ReadAt } from "../data/keyed-read.ts";
import { FakeDeriver, FakeHost } from "../data/fake.test-support.ts";
import { ROUTE_DERIVED, ROUTE_GRAPH, routeDerivedKey, routeGraphKey, type Derived, type RouteFound, type Shown } from "./route-reads.ts";
import { routeScheduleKey } from "./route-views.ts";

const graph = { nodes: [] };
const header = { id: "r_a", name: "A route", kind: "process" as const };
const version = (number: number): RouteVersion => ({ route: "r_a", version: number, published_at: "2026-10-01T00:00:00Z", graph });
const routeOf = (parts: Partial<Route>): Route => ({ header, revision: 3, ...parts });

const at = <T>(): ReadAt<T> => ({ held: undefined, signal: new AbortController().signal, compatible: () => true });

async function found(route: Route, versions: RouteVersion[], wanted?: number): Promise<RouteFound> {
  const host = new FakeHost();
  host.routesHeld.set("r_a", { route, versions });
  return ROUTE_GRAPH.read({ host, id: "r_a", version: wanted }, at());
}

describe("the route graph read", () => {
  it("finds the draft when one is open and no version is asked for", async () => {
    const draft = { graph: { nodes: [], default_owner: "r_x" } };
    const result = await found(routeOf({ draft, versions: [1] }), [version(1)]);
    expect(result).toMatchObject({ shown: { of: "draft", graph: draft.graph } });
  });

  it("finds the version asked for, or the latest when there is no draft", async () => {
    const route = routeOf({ versions: [1, 2] });
    expect(await found(route, [version(1), version(2)], 1)).toMatchObject({ shown: { of: 1 } });
    expect(await found(route, [version(1), version(2)])).toMatchObject({ shown: { of: 2 } });
  });

  it("finds a version even while a draft is open, when one is asked for", async () => {
    const route = routeOf({ draft: { graph }, versions: [1] });
    expect(await found(route, [version(1)], 1)).toMatchObject({ shown: { of: 1 } });
  });

  it("finds nothing to draw for a route whose draft was discarded and that has no version", async () => {
    const route = routeOf({});
    expect(await found(route, [])).toEqual({ empty: route });
  });

  it("holds the revision of the route it found, and is about that route only", () => {
    const route = routeOf({ revision: 7 });
    const ticks = ROUTE_GRAPH.ticks?.({ host: new FakeHost(), id: "r_a", version: undefined });
    expect(ticks?.holds({ empty: route })).toEqual([{ of: { domain: { route: "r_a" } }, revision: 7 }]);
    expect(ticks?.about({ domain: { route: "r_a" } })).toBe(true);
    expect(ticks?.about({ domain: { route: "r_b" } })).toBe(false);
    expect(ticks?.about({ domain: { journey: "r_a" } })).toBe(false);
    expect(ticks?.reportsLag).toBe(true);
    expect(ticks?.subject).toEqual({ domain: { route: "r_a" } });
  });

  it("is the same read for the same route and version, and another for another", () => {
    const host = new FakeHost();
    expect(routeGraphKey(host, "r_a", undefined).of).toEqual(["r_a", undefined]);
    expect(routeGraphKey(host, "r_a", 2).of).toEqual(["r_a", 2]);
    expect(routeGraphKey(host, "r_b", 2).of).not.toEqual(routeGraphKey(host, "r_a", 2).of);
  });
});

/** A deriver that holds what it is asked to, by slot, and records what was loaded and released. */
function holding() {
  const loaded: { graph: unknown; slot: string }[] = [];
  const released: unknown[] = [];
  const deriver = {
    loadRoute: (request: { graph: unknown }, slot: string) => {
      loaded.push({ graph: request.graph, slot });
      return Promise.resolve({ key: { journey: slot, revision: 0, deployment_revision: 4, today: "2000-01-01", generation: loaded.length }, handle: { journey: slot, load: loaded.length } });
    },
    release: (handle: unknown) => { released.push(handle); return Promise.resolve(); },
  } as unknown as Deriver;
  return { deriver, loaded, released };
}

describe("a shown graph, derived and held", () => {
  const deployment: Deployment = { revision: 4 };
  const shown: Shown = { route: routeOf({}), graph, of: "draft" };
  const key = (parts: { shown?: Shown; deployment?: Deployment | undefined }) => routeDerivedKey(new FakeDeriver(), parts.shown ?? shown, "deployment" in parts ? parts.deployment : deployment);

  it("has nothing to read before the deployment is held, or without a graph", () => {
    expect(key({ deployment: undefined })).toBeUndefined();
    expect(routeDerivedKey(new FakeDeriver(), undefined, deployment)).toBeUndefined();
  });

  it("is the reads of one route and what it shows, derived again, keeping what it shows, when the graph or the deployment moves", () => {
    const base = key({});
    const moved = key({ shown: { ...shown, graph: { nodes: [] } } });
    expect(sameKey(moved?.of, base?.of)).toBe(true);
    // Keys compare like effect dependencies: a graph read again is another object, whatever it holds.
    expect(sameKey(moved?.freshness, base?.freshness)).toBe(false);
    expect(sameKey(key({ deployment: { revision: 5 } })?.freshness, base?.freshness)).toBe(false);
    expect(sameKey(key({})?.freshness, base?.freshness)).toBe(true);
  });

  it("is another read for another route, and for a draft and a version of one route", () => {
    const other: Shown = { ...shown, route: routeOf({ header: { ...header, id: "r_b" } }) };
    expect(key({ shown: other })?.of).not.toEqual(key({})?.of);
    expect(key({ shown: { ...shown, of: 2 } })?.of).not.toEqual(key({})?.of);
  });

  it("loads each time under a slot of its own, and answers the graph it derived with its derivation", async () => {
    const { deriver, loaded } = holding();
    const args = routeDerivedKey(deriver, shown, deployment)?.args;
    if (args === undefined) {
      throw new Error("no key");
    }
    const first = await ROUTE_DERIVED.read(args, at());
    const second = await ROUTE_DERIVED.read(args, at());
    expect(first.graph).toBe(graph);
    expect(first.route).toBe(shown.route);
    expect(first.key.journey).toBe(loaded[0]?.slot);
    expect(loaded[0]?.slot).not.toBe(loaded[1]?.slot);
    expect(first.reads).toBe(second.reads);
  });

  it("frees the derivation it loaded once, when its value is dropped or its read is", async () => {
    const { deriver, released } = holding();
    const args = routeDerivedKey(deriver, shown, deployment)?.args;
    if (args === undefined) {
      throw new Error("no key");
    }
    const dropped = await ROUTE_DERIVED.read(args, at());
    ROUTE_DERIVED.release?.(dropped);
    ROUTE_DERIVED.release?.(dropped);
    expect(released).toEqual([{ journey: dropped.key.journey, load: 1 }]);
    const aborted = new AbortController();
    aborted.abort();
    await expect(ROUTE_DERIVED.read(args, { held: undefined, signal: aborted.signal, compatible: () => true })).rejects.toThrow("no longer wanted");
    expect(released).toHaveLength(2);
  });
});

describe("the schedule of a derived graph", () => {
  const derived = { key: { journey: "route:r_a:2#7", revision: 0, deployment_revision: 4, today: "2000-01-01", generation: 3 }, reads: "route:r_a:2" } as unknown as Derived<Shown>;

  it("is of the derivation's reads and read again for each derivation", () => {
    const base = routeScheduleKey(new FakeDeriver(), derived);
    expect(base?.of).toEqual(["route:r_a:2"]);
    const next = { ...derived, key: { ...derived.key, generation: 4 } };
    expect(sameKey(routeScheduleKey(new FakeDeriver(), next)?.of, base?.of)).toBe(true);
    expect(sameKey(routeScheduleKey(new FakeDeriver(), next)?.freshness, base?.freshness)).toBe(false);
    expect(routeScheduleKey(new FakeDeriver(), undefined)).toBeUndefined();
  });
});
