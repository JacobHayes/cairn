// H5: an entity edit kept after a conflict is drafted against the deployment revision the
// rejection reports, so the retry works before the tab hears the tick.
import { describe, expect, it } from "vitest";

import { deploymentRebase } from "./Entities.tsx";

describe("rebasing an entity edit (H5)", () => {
  const stale = (current: number) => ({
    rejection: "stale" as const,
    conflicts: [
      { of: { domain: { journey: "j_other" } }, expected: 1, current: 40 },
      { of: { domain: "deployment" as const }, expected: 3, current },
    ],
    intervening: [],
  });

  it("takes the deployment's revision from the rejection, or the one held if newer", () => {
    expect(deploymentRebase(stale(7), 3)).toBe(7);
    expect(deploymentRebase(stale(7), 9)).toBe(9);
    expect(deploymentRebase({ rejection: "invalid", violations: [] }, 3)).toBe(3);
  });
});
