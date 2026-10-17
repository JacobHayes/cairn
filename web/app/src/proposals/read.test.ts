import { describe, expect, it } from "vitest";

import type { Session } from "../data/session.ts";
import { proposalsListed } from "./read.ts";

const spec = (scope: "waiting" | "new_routes" | "new_journeys") => proposalsListed(scope)({} as Session);

describe("what keeps a list of proposals current (H6)", () => {
  it("refreshes what is waiting on the viewer when a route, a journey or the deployment moves", () => {
    const waiting = spec("waiting");
    expect(waiting.watching).toContain("routes");
    expect(waiting.about({ domain: { route: "r1" } })).toBe(true);
    expect(waiting.about({ domain: { journey: "j1" } })).toBe(true);
    expect(waiting.about({ domain: "deployment" })).toBe(true);
  });

  it("refreshes the proposals that would create a route only for routes", () => {
    const created = spec("new_routes");
    expect(created.about({ domain: { route: "r1" } })).toBe(true);
    expect(created.about({ domain: { journey: "j1" } })).toBe(false);
    expect(created.about({ domain: "deployment" })).toBe(false);
  });
});
