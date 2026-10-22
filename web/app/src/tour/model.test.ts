// Where the tour opens and what stands in its place: Home only, after the name prompt, never
// over a deep link; a run in progress says so one line anywhere but under its journey.
import { describe, expect, it } from "vitest";

import { surfaceOf, type SurfaceInput } from "./model.ts";
import { locate, underJourney } from "./where.ts";

function input(pathname: string, rest: Partial<SurfaceInput> = {}): SurfaceInput {
  return { phase: "idle", failure: undefined, offerable: true, invited: false, unsaved: false, run: undefined, pathname, home: locate(pathname).home, nameSettled: true, ...rest };
}

describe("the welcome", () => {
  it("opens on Home for a record that offers it", () => {
    expect(surfaceOf(input("/"))).toBe("welcome");
  });

  it.each(["/journeys/j_one", "/journeys/j_one/nodes/n_a", "/routes/r_one", "/me", "/identities", "/journeys"])("never opens over %s", (path) => {
    expect(surfaceOf(input(path))).toBe("none");
  });

  it("waits for the name prompt", () => {
    expect(surfaceOf(input("/", { nameSettled: false }))).toBe("none");
  });

  it("stays shut for a record that does not offer it, unless the person asked, and while a closing write is unsaved", () => {
    expect(surfaceOf(input("/", { offerable: false }))).toBe("none");
    expect(surfaceOf(input("/", { offerable: false, invited: true }))).toBe("welcome");
    expect(surfaceOf(input("/", { unsaved: true }))).toBe("none");
  });
});

describe("a run in progress", () => {
  const running = { phase: "running" as const, run: "j_practice" };

  it("shows its step under the practice journey and its nodes", () => {
    expect(surfaceOf(input("/journeys/j_practice", running))).toBe("band");
    expect(surfaceOf(input("/journeys/j_practice/nodes/n_a", running))).toBe("band");
  });

  it("offers to continue on Home, and says it is paused anywhere else", () => {
    expect(surfaceOf(input("/", running))).toBe("continue");
    expect(surfaceOf(input("/journeys/j_other", running))).toBe("paused");
    expect(surfaceOf(input("/routes", running))).toBe("paused");
  });

  it("is not another journey whose id starts with the practice journey's", () => {
    expect(underJourney("/journeys/j_practice_two", "j_practice")).toBe(false);
  });
});

describe("failures and the end", () => {
  it("say so on Home, where the tour was", () => {
    expect(surfaceOf(input("/", { phase: "failed", failure: "start" }))).toBe("start-failed");
    expect(surfaceOf(input("/", { phase: "failed", failure: "cleanup" }))).toBe("cleanup-failed");
    expect(surfaceOf(input("/", { phase: "ended" }))).toBe("end");
    expect(surfaceOf(input("/me", { phase: "ended" }))).toBe("none");
  });
});
