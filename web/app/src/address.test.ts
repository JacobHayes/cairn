import { describe, expect, it } from "vitest";

import { ADDRESSES, addressOf, pathTo, proposalAddress, proposalIn, sectionOf, type AddressName } from "./address.ts";

const SAMPLE = { journey: "j_a", route: "r_a", proposal: "pr_a", key: "n_x", edge: "n_x~n_y" };

/** Each pattern's parameters, filled from the sample. */
function paramsFor(path: string): Record<string, string> {
  return Object.fromEntries([...path.matchAll(/:(\w+)/g)].map(([, name = ""]) => [name, SAMPLE[name as keyof typeof SAMPLE]]));
}

describe("the address table", () => {
  it.each(ADDRESSES.map((each): [AddressName] => [each.name]))("%s is read back as itself from the path built from it", (name) => {
    const entry = ADDRESSES.find((each) => each.name === name);
    const params = paramsFor(entry?.path ?? "");
    const path = pathTo(name, params as never);
    const found = addressOf(path);
    expect(found?.name).toBe(name);
    expect(found?.params).toEqual(params);
  });

  it("reads the static pages before the ones with a parameter in their place", () => {
    expect(addressOf("/journeys/new")?.name).toBe("journeyNew");
    expect(addressOf("/routes/new")?.name).toBe("routeNew");
    expect(addressOf("/journeys/new/proposals")?.name).toBe("journeyProposals");
  });

  it("knows no page for an address that is not one, including the proposal addresses of earlier builds", () => {
    expect(addressOf("/proposals/pr_a")).toBeUndefined();
    expect(addressOf("/journeys/j_a/other")).toBeUndefined();
  });

  it("puts a proposal under its destination, in that destination's section of the rail", () => {
    expect(proposalAddress({ journey: "j_a" }, "pr_a")).toBe("/journeys/j_a/proposals/pr_a");
    expect(proposalAddress({ route: "r_a" }, "pr_a", "n_x")).toBe("/routes/r_a/proposals/pr_a/nodes/n_x");
    expect(proposalAddress("deployment", "pr_a")).toBe("/entities/proposals/pr_a");
    expect(sectionOf("/routes/r_a/proposals/pr_a")).toBe("routes");
    expect(sectionOf("/entities/proposals/pr_a")).toBe("entities");
    const found = addressOf("/routes/r_a/proposals/pr_a/nodes/n_x");
    expect(found === undefined ? undefined : proposalIn(found)).toEqual({ destination: { route: "r_a" }, proposal: "pr_a", key: "n_x" });
  });
});
