// C17: a route's page address: a version is written whenever a link names one (its link must keep
// meaning it as drafts open and versions publish), the route as it stands writes none, the view
// only off the Graph, a node only on the route as it stands, and the shared view state and the
// graph's own settings ride along through every switch.
import { describe, expect, it } from "vitest";

import { routePath, routeScreenOf, routeSwitched, spotOf, versionIn } from "./address.ts";

const query = (path: string) => new URLSearchParams(path.split("?")[1] ?? "");

describe("a route's address", () => {
  it("writes the version a link names, the latest included, and none for the route as it stands", () => {
    expect(routePath("r")).toBe("/routes/r");
    expect(routePath("r", { version: 3 })).toBe("/routes/r?version=3");
    expect(routePath("r", { version: 1, screen: "list" })).toBe("/routes/r?version=1&view=list");
  });

  it("names the Graph only by leaving the view out", () => {
    expect(routePath("r", { screen: "graph" })).toBe("/routes/r");
    expect(query(routePath("r", { screen: "timeline" })).get("view")).toBe("timeline");
  });

  it("puts a node in the path, and keeps the version a link names beside it", () => {
    expect(routePath("r", { node: "n_x" })).toBe("/routes/r/nodes/n_x");
    expect(routePath("r", { node: "n_x", version: 2 })).toBe("/routes/r/nodes/n_x?version=2");
  });

  it("takes the version and the view from the spot, never from the query it keeps", () => {
    const path = routePath("r", { version: 2 }, "?version=1&view=list&depth=stages&q=a&open=n_s");
    expect(query(path).getAll("version")).toEqual(["2"]);
    expect(query(path).has("view")).toBe(false);
    expect([...query(path).keys()]).toEqual(["version", "depth", "q", "open"]);
  });

  it("reads the version as a whole number from 1, and the view among a route's", () => {
    expect(versionIn(new URLSearchParams("version=2"))).toBe(2);
    for (const bad of ["", "0", "-1", "1.5", "x"]) {
      expect(versionIn(new URLSearchParams(`version=${bad}`))).toBeUndefined();
    }
    expect(versionIn(new URLSearchParams(""))).toBeUndefined();
    expect(routeScreenOf(new URLSearchParams("view=timeline"))).toBe("timeline");
    // A journey's views are not a route's.
    expect(routeScreenOf(new URLSearchParams("view=summary"))).toBe("graph");
    expect(routeScreenOf(new URLSearchParams("view=walk"))).toBe("graph");
    expect(spotOf(new URLSearchParams("version=2&view=list"))).toEqual({ version: 2, screen: "list" });
  });
});

describe("switching a route's view", () => {
  const search = "?version=2&view=list&depth=decisions&fade=1&q=ship&open=n_a&shut=n_b";

  it("keeps the version, the view state and the graph's settings", () => {
    const to = query(routeSwitched("r", search, "timeline", undefined));
    expect(Object.fromEntries(to)).toEqual({ version: "2", view: "timeline", depth: "decisions", fade: "1", q: "ship", open: "n_a", shut: "n_b" });
    const back = query(routeSwitched("r", `?${to.toString()}`, "graph", undefined));
    expect(Object.fromEntries(back)).toEqual({ version: "2", depth: "decisions", fade: "1", q: "ship", open: "n_a", shut: "n_b" });
  });

  it("keeps the node open, on a draft and on a version", () => {
    expect(routeSwitched("r", "?view=list", "timeline", "n_x")).toBe("/routes/r/nodes/n_x?view=timeline");
    expect(routeSwitched("r", "?version=2", "list", "n_x")).toBe("/routes/r/nodes/n_x?version=2&view=list");
  });

  it("stays under a proposal's layer, the layer's own address and the picked node kept", () => {
    expect(routeSwitched("r", "?view=list", "timeline", undefined, "pr_1")).toBe("/routes/r/proposals/pr_1?view=timeline");
    expect(routeSwitched("r", "", "list", "n_x", "pr_1")).toBe("/routes/r/proposals/pr_1/nodes/n_x?view=list");
  });
});
