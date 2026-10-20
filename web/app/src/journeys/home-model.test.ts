// C16: Home's four states and what it counts, from what each candidate journey read.
import { describe, expect, it } from "vitest";

import { actingView } from "../acting/acting.test-support.ts";
import type { NodeRow } from "../acting/why.ts";
import { heldBack, homeState, readyLine, type HomeJourney } from "./home-model.ts";

const linked = { identities: 1 };
const none = { waiting: 0 };
const journey = (reading: HomeJourney["reading"], participates = false, ready = 0): HomeJourney => ({ reading, participates, ready });

describe("homeState", () => {
  const cases: [string, Parameters<typeof homeState>[0], ReturnType<typeof homeState>][] = [
    ["the viewer not yet read", { viewer: "loading", index: "ready", journeys: [], proposals: none }, "loading"],
    ["the index not yet read", { viewer: linked, index: "loading", journeys: [], proposals: none }, "loading"],
    ["a viewer that could not be read", { viewer: "failed", index: "ready", journeys: [], proposals: none }, "failed"],
    ["an index that could not be read", { viewer: linked, index: "failed", journeys: [], proposals: none }, "failed"],
    ["no identity of the viewer's", { viewer: { identities: 0 }, index: "ready", journeys: [], proposals: none }, "unlinked"],
    ["identities and no active journey referring to them", { viewer: linked, index: "ready", journeys: [], proposals: none }, "unassigned"],
    ["journeys read, none of them holding anything of theirs", { viewer: linked, index: "ready", journeys: [journey("ready"), journey("gone")], proposals: none }, "unassigned"],
    ["work that is all waiting", { viewer: linked, index: "ready", journeys: [journey("ready", true, 0)], proposals: none }, "quiet"],
    ["only finished participation", { viewer: linked, index: "ready", journeys: [journey("ready", true, 0), journey("ready", false, 0)], proposals: none }, "quiet"],
    ["some work that can move", { viewer: linked, index: "ready", journeys: [journey("ready", true, 2), journey("loading")], proposals: none }, "ready"],
    ["journeys still being read, none with anything yet", { viewer: linked, index: "ready", journeys: [journey("ready"), journey("loading")], proposals: none }, "loading"],
    ["proposals waiting on them, though no work can move", { viewer: linked, index: "ready", journeys: [journey("ready", true, 0)], proposals: { waiting: 2 } }, "ready"],
    ["proposals not yet read, with nothing else to show", { viewer: linked, index: "ready", journeys: [journey("ready", true, 0)], proposals: "loading" }, "loading"],
    ["proposals that could not be read are not none", { viewer: linked, index: "ready", journeys: [journey("ready")], proposals: "failed" }, "failed"],
    ["a journey that failed is not no work", { viewer: linked, index: "ready", journeys: [journey("ready"), journey("failed")], proposals: none }, "ready"],
  ];
  it.each(cases)("%s", (_name, input, expected) => {
    expect(homeState(input)).toBe(expected);
  });
});

describe("the line under the title", () => {
  it("counts the rows and the journeys once every journey is read", () => {
    expect(readyLine([journey("ready", true, 2), journey("ready", true, 4), journey("ready", true, 0)])).toEqual({ rows: 6, journeys: 2 });
  });

  it("says nothing while any journey is still read or failed, and when nothing is ready", () => {
    expect(readyLine([journey("ready", true, 2), journey("loading")])).toBeUndefined();
    expect(readyLine([journey("ready", true, 2), journey("failed")])).toBeUndefined();
    expect(readyLine([journey("ready", true, 0)])).toBeUndefined();
  });
});

describe("what of the viewer's is held back", () => {
  const rows = (...keys: string[]) => keys.map((key) => ({ key }) as NodeRow);
  const mine = (...nodes: string[]) => nodes.map((node) => ({ node, kinds: ["k_owner"] }));

  it("counts their open work that cannot move yet, and how much of it is late", () => {
    const view = actingView();
    const later = view.derived.nodes["n_later"];
    if (later !== undefined) {
      later.overdue = true;
    }
    expect(heldBack(view, mine("n_pick", "n_later", "n_inner", "n_done", "n_stage"), rows("n_pick"))).toEqual({ count: 2, late: 1 });
  });
});
