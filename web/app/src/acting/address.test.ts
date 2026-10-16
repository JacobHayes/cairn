// The list's own settings in the address: they survive the round trip beside the view state,
// the defaults leave the address bare, and values the list does not know are dropped (C9); and
// the walkthrough's address (C11).
import { describe, expect, it } from "vitest";

import { viewStateFrom } from "../journeys/view.ts";
import { listSettingsFrom, walkthroughPath, withListSettings, type ListSettings } from "./address.ts";

describe("the list's own settings (C9)", () => {
  it("keep the sort and columns, and leave the view state and the rest of the address alone", () => {
    const list: ListSettings = { sort: "slack", columns: ["rank", "unlocks"] };
    const search = withListSettings("?frontier=1&q=plan&sort=due&cols=kind&open=n_a", list);
    expect(listSettingsFrom(new URLSearchParams(search))).toEqual(list);
    expect(viewStateFrom(new URLSearchParams(search))).toEqual(viewStateFrom(new URLSearchParams("?frontier=1&q=plan")));
    expect(new URLSearchParams(search).get("open")).toBe("n_a");
  });

  it("are bare at the defaults, and drop sorts and columns the list does not know", () => {
    expect(withListSettings("?sort=due&cols=kind", { sort: undefined, columns: [] })).toBe("");
    expect(listSettingsFrom(new URLSearchParams("sort=weight&cols=rank,bogus"))).toEqual({ sort: undefined, columns: ["rank"] });
  });
});

describe("the walkthrough's address (C11)", () => {
  it("opens the walk through at Decisions depth", () => {
    const [path = "", query = ""] = walkthroughPath("j_new").split("?");
    expect(path).toBe("/journeys/j_new/walk");
    expect(viewStateFrom(new URLSearchParams(query)).depth).toBe("decisions");
  });
});
