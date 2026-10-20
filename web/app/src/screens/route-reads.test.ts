// A route canvas's reads (ARCHITECTURE, Web UI: reads): what the route graph read finds for a
// draft, a version and a route with nothing to draw, which ticks keep it current and what it
// holds, and what makes the level of a shown graph the same read or another.
import { describe, expect, it } from "vitest";

import type { Deployment, Deriver, Route, RouteVersion } from "../data/host.ts";
import { sameKey, type ReadAt } from "../data/keyed-read.ts";
import { levelQueryOf, levelRequestOf } from "../journeys/requests.ts";
import { DEFAULT_VIEW_STATE } from "../journeys/view.ts";
import { FakeDeriver, FakeHost } from "../data/fake.test-support.ts";
import { ROUTE_GRAPH, ROUTE_LEVEL, ROUTE_SCHEDULE, routeGraphKey, routeLevelKey, routeScheduleKey, type RouteFound, type Shown } from "./route-reads.ts";

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

describe("the level of a shown graph", () => {
  const deployment: Deployment = { revision: 4 };
  const shown: Shown = { route: routeOf({}), graph, of: "draft" };
  const request = (step: "decisions" | "stages" | "actions", text = "") => levelRequestOf({ ...DEFAULT_VIEW_STATE, text }, step, []);
  const key = (parts: { shown?: Shown; step?: "decisions" | "stages" | "actions"; text?: string; deployment?: Deployment | undefined; today?: string }) =>
    routeLevelKey(new FakeDeriver(), parts.shown ?? shown, request(parts.step ?? "decisions", parts.text), "deployment" in parts ? parts.deployment : deployment, parts.today ?? "2026-10-16");

  it("has nothing to read before the deployment is held, or without a request", () => {
    expect(key({ deployment: undefined })).toBeUndefined();
    expect(routeLevelKey(new FakeDeriver(), shown, undefined, deployment, "2026-10-16")).toBeUndefined();
  });

  it("is another read for another request of the same graph, laid out afresh", () => {
    expect(key({ step: "stages" })?.of).not.toEqual(key({})?.of);
    expect(key({ text: "ship" })?.of).not.toEqual(key({})?.of);
  });

  it("is read again, keeping what it shows, when the graph, the deployment or the day moves", () => {
    const base = key({});
    const moved = key({ shown: { ...shown, graph: { nodes: [] } } });
    expect(sameKey(moved?.of, base?.of)).toBe(true);
    // Keys compare like effect dependencies: a graph read again is another object, whatever it holds.
    expect(sameKey(moved?.freshness, base?.freshness)).toBe(false);
    expect(sameKey(key({ deployment: { revision: 5 } })?.freshness, base?.freshness)).toBe(false);
    expect(sameKey(key({ today: "2026-10-17" })?.freshness, base?.freshness)).toBe(false);
    expect(sameKey(key({})?.freshness, base?.freshness)).toBe(true);
  });

  it("is another read for the same request of another route's graph", () => {
    const other: Shown = { ...shown, route: routeOf({ header: { ...header, id: "r_b" } }) };
    expect(key({ shown: other })?.of).not.toEqual(key({})?.of);
  });

  it("is derived from the graph and the day with the request's query, whole and without its tag", async () => {
    const asked: unknown[] = [];
    const answer = { nodes: [], edges: [], shown: [] };
    const deriver = { routeLevel: (derive: unknown) => { asked.push(derive); return Promise.resolve(answer); } } as unknown as Deriver;
    const args = routeLevelKey(deriver, shown, request("stages", "ship"), deployment, "2026-10-16")?.args;
    if (args === undefined) {
      throw new Error("no key");
    }
    expect(await ROUTE_LEVEL.read(args, at())).toBe(answer);
    const query = levelQueryOf(request("stages", "ship"));
    expect(asked).toEqual([{ graph, deployment, today: "2026-10-16", query }]);
    expect(query).not.toHaveProperty("projection");
    expect(query).toMatchObject({ text: "ship", top: ["group", "milestone"] });
  });
});

describe("the schedule of a shown graph", () => {
  const deployment: Deployment = { revision: 4 };
  const shown: Shown = { route: routeOf({}), graph, of: 2 };

  it("is keyed by the route and what it shows, and read again when the graph or the deployment moves", () => {
    const base = routeScheduleKey(new FakeDeriver(), shown, deployment);
    expect(base?.of).toEqual(["r_a", 2]);
    expect(routeScheduleKey(new FakeDeriver(), { ...shown, of: 1 }, deployment)?.of).not.toEqual(base?.of);
    expect(sameKey(routeScheduleKey(new FakeDeriver(), { ...shown, graph: { nodes: [] } }, deployment)?.freshness, base?.freshness)).toBe(false);
    expect(sameKey(routeScheduleKey(new FakeDeriver(), shown, { revision: 5 })?.freshness, base?.freshness)).toBe(false);
    expect(routeScheduleKey(new FakeDeriver(), shown, undefined)).toBeUndefined();
  });

  it("asks the deriver for the shown graph's schedule", async () => {
    const asked: unknown[] = [];
    const schedule = { entries: [] };
    const deriver = { routeSchedule: (derive: unknown) => { asked.push(derive); return Promise.resolve(schedule); } } as unknown as Deriver;
    const args = routeScheduleKey(deriver, shown, deployment)?.args;
    if (args === undefined) {
      throw new Error("no key");
    }
    expect(await ROUTE_SCHEDULE.read(args, at())).toBe(schedule);
    expect(asked).toEqual([{ graph, deployment }]);
  });
});
