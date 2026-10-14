// A screen comes back as it was left, except for the graph's modes; the tree's levels are kept per journey.
import { expect, it } from "vitest";

import { keepFolds, keepQuery, keepViewport, keptFolds, keptQuery, keptViewport, switchedSearch } from "./kept.ts";

it("returns to a screen's own settings, not to its modes", () => {
  keepQuery("j_a", "graph", "?open=n_setup&edit=on&select=1&map=1&frontier=1");
  expect(keptQuery("j_a", "graph")).toBe("open=n_setup&frontier=1");
  expect(switchedSearch("j_a", "?sort=due", "list", "graph")).toBe("?open=n_setup");
  expect(switchedSearch("j_b", "", "list", "graph")).toBe("");
  // An empty setting means something (no conditional nodes, no kinds), and the screen already open is not rewritten from memory.
  keepQuery("j_a", "graph", "?show=&kind=");
  expect(switchedSearch("j_a", "", "list", "graph")).toBe("?show=&kind=");
  expect(switchedSearch("j_a", "?flag=a", "walk", "walk")).toBe("?flag=a");
});

it("keeps the list's levels for the journey that opened them", () => {
  keepFolds("j_a", { by: new Map([["n_stage", true]]), every: undefined });
  expect(keptFolds("j_a").by.get("n_stage")).toBe(true);
  expect(keptFolds("j_b").by.size).toBe(0);
});

it("keeps a canvas's viewport for its journey at the level it was left on", () => {
  keepViewport("j_a/work", { x: 4, y: 5, zoom: 0.5 });
  expect(keptViewport("j_a/work")).toEqual({ x: 4, y: 5, zoom: 0.5 });
  expect(keptViewport("j_a/stages")).toBeUndefined();
});
