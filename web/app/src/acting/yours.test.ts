// C10, C16: the line for a viewer whose own work is all waiting names what is theirs next and what it waits on.
import { describe, expect, it } from "vitest";

import { actingView } from "./acting.test-support.ts";
import { nextForYou } from "./yours.ts";

const holds = (...nodes: string[]) => nodes.map((node) => ({ node, kinds: ["k_owner"] }));

describe("nextForYou", () => {
  it("is silent while something of theirs is on the frontier", () => {
    expect(nextForYou(actingView(), holds("n_pick", "n_later"))).toBeUndefined();
  });

  it("names their earliest-starting open node and what it waits on", () => {
    const view = actingView();
    // n_later starts Oct 20 after the meeting; n_inner starts Oct 13 behind the stage's opening.
    const next = nextForYou(view, holds("n_later", "n_inner"));
    expect(next).toMatchObject({ node: "n_inner", after: ["Meeting"] });
  });
});
