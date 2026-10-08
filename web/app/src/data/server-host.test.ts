// C16: the journey index's query as `GET /api/journeys` reads it: repeated statuses and entities,
// and nothing for a filter left empty.
import { describe, expect, it } from "vitest";

import { indexParams } from "./server-host.ts";

describe("the index's query parameters", () => {
  it("names each filter given and leaves out the empty ones", () => {
    expect(indexParams({ status: ["active"], route: "r", version: 2, referencing: ["e_a", "e_b"], upgrade_available: false, after: "j_x" })).toEqual({
      status: ["active"],
      route: "r",
      version: 2,
      referencing: ["e_a", "e_b"],
      upgrade_available: false,
      after: "j_x",
    });
    expect(indexParams({ status: [], referencing: [] })).toEqual({});
    expect(indexParams()).toEqual({});
  });
});
