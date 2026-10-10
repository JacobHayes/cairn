// The acting surfaces' addresses: every setting survives the round trip, the defaults leave
// the address bare, values the screen does not know are dropped, and each screen's settings
// become the engine's query (C9, C10, C11).
import { describe, expect, it } from "vitest";

import {
  DEFAULT_LIST,
  DEFAULT_NEXT,
  DEFAULT_TRIAGE,
  listFrom,
  listParams,
  listQueryOf,
  nextFrom,
  nextParams,
  nextQueryOf,
  triageFrom,
  triageParams,
  triageQueryOf,
  nextPath,
  walkthroughPath,
  type ListSettings,
} from "./address.ts";

const everything: ListSettings = {
  flags: ["mine", "overdue", "snoozed_and_overdue"],
  within: "n_setup",
  owner: "e_lead",
  states: ["active", "ready"],
  notRelevant: true,
  kinds: ["deliverable", "action"],
  text: "plan",
  sort: "slack",
  columns: ["rank", "unblocks"],
  decisions: false,
};

describe("the list's address (C9)", () => {
  it("keeps every setting", () => {
    expect(listFrom(listParams(everything))).toEqual(everything);
    expect(listFrom(listParams({ ...everything, decisions: true }))).toEqual({ ...everything, decisions: true });
  });

  it("is bare at the defaults", () => {
    expect(listParams(DEFAULT_LIST).toString()).toBe("");
  });

  it("leaves not-relevant out of the display states it asks for, unless asked", () => {
    expect(listQueryOf({ ...DEFAULT_LIST, notRelevant: true }).display_states).toEqual([]);
    expect(listQueryOf({ ...DEFAULT_LIST, states: ["done"], notRelevant: true }).display_states).toEqual(["done", "not_relevant"]);
  });

  it("writes the mine flag as the chip's own parameter, and reads either", () => {
    const params = listParams({ ...DEFAULT_LIST, flags: ["mine", "overdue"] });
    expect(params.toString()).toBe("mine=1&flag=overdue");
    expect(listFrom(new URLSearchParams("flag=mine")).flags).toEqual(["mine"]);
    expect(listFrom(new URLSearchParams("mine=1&flag=overdue")).flags).toEqual(["mine", "overdue"]);
  });

  it("drops filters, states, kinds, and sorts it does not know", () => {
    const settings = listFrom(new URLSearchParams("flag=mine,bogus&state=done,lost&kind=group,shape&sort=weight&cols=rank,bogus"));
    expect([settings.flags, settings.states, settings.kinds, settings.sort, settings.columns]).toEqual([["mine"], ["done"], ["group"], undefined, ["rank"]]);
  });

  it("asks the engine for every filter at once, a page from its cursor", () => {
    expect(listQueryOf({ ...everything, text: "  plan  " }, 200)).toEqual({
      flags: everything.flags,
      within: "n_setup",
      owner: "e_lead",
      display_states: ["active", "ready", "not_relevant"],
      kinds: everything.kinds,
      text: "plan",
      sort: "slack",
      cursor: 200,
    });
  });

  it("asks for no text when the search is blank", () => {
    expect(listQueryOf({ ...DEFAULT_LIST, text: "   " })).not.toHaveProperty("text");
  });
});

describe("the next list's address (C10)", () => {
  const settings = { sort: "leverage" as const, mine: true, kinds: ["decision" as const, "milestone" as const], flags: ["overdue" as const, "unassigned" as const], forMe: true, decisions: false, text: "plan" };

  it("keeps every setting, and is bare at the defaults", () => {
    expect(nextFrom(nextParams(settings))).toEqual(settings);
    expect(nextParams(DEFAULT_NEXT).toString()).toBe("");
  });

  it("narrows to the decisions when DECISIONS is on, in the next list and the list alike", () => {
    expect(nextQueryOf({ ...settings, decisions: true }).kinds).toEqual(["decision"]);
    expect(listQueryOf({ ...everything, decisions: true }).kinds).toEqual(["decision"]);
    expect(listQueryOf({ ...everything, decisions: false }).kinds).toEqual(["deliverable", "action"]);
  });

  it("asks for prioritize for me as the ranking for the viewer", () => {
    expect(nextQueryOf(settings)).toEqual({ sort: "leverage", mine: true, kinds: ["decision", "milestone"], for_viewer: true });
  });

  it("offers no group kind: groups are never on the frontier", () => {
    expect(nextFrom(new URLSearchParams("kind=group,action")).kinds).toEqual(["action"]);
  });

  it("opens a node beside the list without touching what the list asks for", () => {
    const [bare, beside] = [nextPath("j_a", settings), nextPath("j_a", settings, "n_x")];
    expect(beside).toBe(bare.replace("/next/list", "/next/list/nodes/n_x"));
    expect(nextQueryOf(nextFrom(new URLSearchParams(beside.split("?")[1])))).toEqual(nextQueryOf(settings));
  });

  it("reads only the flags a next row carries; the engine's query takes none", () => {
    expect(nextFrom(new URLSearchParams("flag=stale,snoozed,next_up,overdue")).flags).toEqual(["overdue", "stale"]);
    expect(nextQueryOf(settings)).not.toHaveProperty("flags");
  });
});

describe("triage's address (C11)", () => {
  it("keeps the mode and filters, and is bare at the defaults", () => {
    const settings = { decisions: false, mine: true, kinds: ["action" as const], flags: ["stale" as const], text: "" };
    expect(triageFrom(triageParams(settings))).toEqual(settings);
    expect(triageParams(DEFAULT_TRIAGE).toString()).toBe("");
  });

  it("reads the frontier in rank order, decisions only in the walkthrough", () => {
    expect(triageQueryOf({ decisions: true, mine: false, kinds: ["action"], flags: [], text: "" })).toEqual({ sort: "rank", mine: false, kinds: ["decision"] });
    expect(triageQueryOf({ decisions: false, mine: true, kinds: [], flags: [], text: "" })).toEqual({ sort: "rank", mine: true, kinds: [] });
  });

  it("opens the walkthrough on the cards with DECISIONS on", () => {
    const [path = "", query = ""] = walkthroughPath("j_new").split("?");
    expect(path).toBe("/journeys/j_new/next/cards");
    expect(query).toBe("decisions=1");
    expect(triageFrom(new URLSearchParams(query)).decisions).toBe(true);
  });
});
