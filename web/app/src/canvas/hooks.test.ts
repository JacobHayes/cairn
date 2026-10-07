// A projection's answer is shown only for the request it answered: an earlier revision's
// answer to the same request stays until the next arrives, another request's never does.
import { describe, expect, test } from "vitest";

import { heldFor } from "./hooks.ts";

describe("what a canvas read shows", () => {
  const level = JSON.stringify(["j_one", { projection: "level", shown: ["group"] }]);
  const other = JSON.stringify(["j_one", { projection: "level", shown: ["action"] }]);

  test.each([
    ["the request it answered", level, 1],
    ["another request", other, undefined],
    ["no request", undefined, undefined],
  ])("for %s", (_, request, value) => {
    expect(heldFor({ request: level, value: 1 }, request).value).toBe(value);
  });

  test("an error is shown only for its own request", () => {
    expect(heldFor({ request: level, error: "no node" }, level).error).toBe("no node");
    expect(heldFor({ request: level, error: "no node" }, other).error).toBeUndefined();
  });
});
