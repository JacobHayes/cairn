// The owner slot says the same thing on every surface: the viewer is "You" wherever other people are named.
import { OWNER_KIND } from "@cairn/client";
import { describe, expect, it } from "vitest";

import type { Ready } from "./model.ts";
import { ownerName } from "./sentence.ts";
import { testView } from "./view.test-support.ts";

function owned(by: string[]): Ready {
  const view = testView();
  const node = view.derived.nodes.n_report;
  if (node === undefined) {
    throw new Error("the report has derived values");
  }
  return { ...view, derived: { ...view.derived, nodes: { ...view.derived.nodes, n_report: { ...node, participations: { [OWNER_KIND.key]: { identities: by, origin: "explicit" } } } } } };
}

describe("ownerName", () => {
  it("names the owners when the viewer is not among them", () => {
    expect(ownerName(owned(["who_two"]), "n_report", ["who_one"])).toBe("Person Two");
  });

  it("says You for the viewer, ahead of the others, as the canvas card does", () => {
    expect(ownerName(owned(["who_two", "who_one"]), "n_report", ["who_one"])).toBe("You and Person Two");
    expect(ownerName(owned(["who_one"]), "n_report", ["who_one"])).toBe("You");
  });

  it("says nothing for a node nobody owns", () => {
    expect(ownerName(testView(), "n_report", ["who_one"])).toBeUndefined();
  });
});
