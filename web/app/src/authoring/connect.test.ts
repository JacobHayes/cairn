// What the edge drawing says when a picked card's write does not land.
import { describe, expect, it } from "vitest";

import type { Rejection } from "../data/writes.ts";
import { pickProblem } from "./connect.tsx";

describe("pickProblem", () => {
  const sent = { mutations: [], places: [] };
  const stale = { failed: { sent, rejection: { rejection: "stale", conflicts: [], intervening: [] } as Rejection }, unanswered: undefined };

  it("says nothing until a pick was dropped, and leaves a rule violation to the banner's own list", () => {
    expect(pickProblem(false, stale)).toBeUndefined();
    expect(pickProblem(true, { failed: { sent, rejection: { rejection: "invalid", violations: [] } as unknown as Rejection }, unanswered: undefined })).toBeUndefined();
  });

  it("tells a rejection, an unconfirmed write and a stopped write apart", () => {
    const [rejected, unconfirmed, stopped, other] = [
      pickProblem(true, stale),
      pickProblem(true, { failed: undefined, unanswered: "failed" }),
      pickProblem(true, { failed: undefined, unanswered: "stopped" }),
      pickProblem(true, { failed: undefined, unanswered: undefined }),
    ];
    expect(rejected).toContain("changed this");
    expect(unconfirmed).toContain("may have been saved");
    expect(stopped).toContain("Reload");
    expect(new Set([rejected, unconfirmed, stopped, other]).size).toBe(4);
  });
});
