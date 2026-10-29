// H5: an identity edit kept after a conflict is drafted against the deployment revision the
// rejection reports, so the retry works before the tab hears the tick. E6, H3: the form sends an
// identity that names someone, within the limits, with no email still being typed.
import { describe, expect, it } from "vitest";

import { deploymentRebase, sendable } from "./IdentityForm.tsx";

describe("rebasing an identity edit (H5)", () => {
  const stale = (current: number) => ({
    rejection: "stale" as const,
    conflicts: [
      { of: { domain: { journey: "j_other" } }, expected: 1, current: 40 },
      { of: { domain: "deployment" as const }, expected: 3, current },
    ],
    intervening: [],
    touched: [],
  });

  it("takes the deployment's revision from the rejection, or the one held if newer", () => {
    expect(deploymentRebase(stale(7), 3)).toBe(7);
    expect(deploymentRebase(stale(7), 9)).toBe(9);
    expect(deploymentRebase({ rejection: "invalid", violations: [] }, 3)).toBe(3);
  });
});

describe("what the form may send (E6, H3)", () => {
  it.each([
    [{ name: "Ana", emails: [], emailText: "" }, true],
    [{ name: " ", emails: ["a@example.org"], emailText: "" }, true],
    [{ name: "", emails: [], emailText: "" }, false],
    [{ name: "  ", emails: [], emailText: "" }, false],
    [{ name: "Ana", emails: [], emailText: "ana@" }, false],
  ])("%j: %s", (typed, can) => {
    expect(sendable(typed)).toBe(can);
  });
});
