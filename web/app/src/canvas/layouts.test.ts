// C15: a view's layouts in this tab. An edit to a view is laid out hinted by its last positions,
// so few nodes move; another level is another view, laid out cold, so a level opened by a click
// is placed as the same level opened directly.
import { expect, test } from "vitest";

import type { Layout, LayoutRequest } from "./layout.ts";
import { Layouts } from "./layouts.ts";
import { DEFAULT_VIEW, layoutViewOf } from "./settings.ts";

const request: Omit<LayoutRequest, "hints"> = { nodes: [{ key: "n_a", width: 10, height: 10 }], edges: [] };
const laid = (): Layout => ({ nodes: { n_a: { x: 5, y: 7, width: 10, height: 10 } }, routes: {} });

test("an edit to a view is hinted by its last positions, and another level is laid out cold", async () => {
  const seen: LayoutRequest[] = [];
  const layouts = new Layouts({
    layOut: (asked) => {
      seen.push(asked);
      return Promise.resolve(laid());
    },
  });
  await layouts.place("j_a", "stages", request);
  await layouts.place("j_a", "work", request);
  await layouts.place("j_a", "stages", { ...request, edges: [{ from: "n_a", to: "n_b" }] });
  expect(seen.map((asked) => asked.hints)).toEqual([undefined, undefined, { n_a: { x: 5, y: 7 } }]);
});

test("a level is its own view, and expanding a container within it is not", () => {
  const work = layoutViewOf(DEFAULT_VIEW, "work");
  expect(layoutViewOf(DEFAULT_VIEW, "stages")).not.toBe(work);
  expect(layoutViewOf({ ...DEFAULT_VIEW, container: "n_setup" }, "work")).not.toBe(work);
  expect(layoutViewOf({ ...DEFAULT_VIEW, shut: ["n_setup"] }, "work")).toBe(work);
});
