import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter, Route, Routes } from "react-router";
import { describe, expect, it } from "vitest";

import { ADDRESSES } from "../address.ts";
import { screenSwitched, type JourneyTarget } from "./address.ts";
import { useJourneyTarget } from "./useJourneyTarget.ts";

/** The target the hook gives at `address`, under the journey page routes the app declares (main.tsx). */
function targetAt(address: string): JourneyTarget {
  let seen: JourneyTarget | undefined;
  function Probe() {
    seen = useJourneyTarget();
    return null;
  }
  renderToStaticMarkup(
    <MemoryRouter initialEntries={[address]}>
      <Routes>
        {ADDRESSES.filter(({ name }) => name === "journey" || name === "journeyNode" || name === "journeyEdge").map(({ name, path }) => (
          <Route key={name} path={path.slice(1)} element={<Probe />} />
        ))}
      </Routes>
    </MemoryRouter>,
  );
  if (seen === undefined) {
    throw new Error(`no journey page at ${address}`);
  }
  return seen;
}

describe("the journey target of a matched address", () => {
  it("is what the router matched, a trailing slash or not", () => {
    for (const slash of ["", "/"]) {
      expect(targetAt(`/journeys/j_a${slash}`)).toEqual({ journey: "j_a", node: undefined, edge: undefined });
      expect(targetAt(`/journeys/j_a/nodes/n_x${slash}`)).toEqual({ journey: "j_a", node: "n_x", edge: undefined });
      expect(targetAt(`/journeys/j_a/edges/n_x~n_y${slash}`)).toEqual({ journey: "j_a", node: undefined, edge: "n_x~n_y" });
    }
  });

  it("switches views from a slash-ended address the way it does from the bare one", () => {
    expect(screenSwitched(targetAt("/journeys/j_a/"), "", "list")).toBe(screenSwitched(targetAt("/journeys/j_a"), "", "list"));
    expect(screenSwitched(targetAt("/journeys/j_a/"), "", "graph")).toBe("/journeys/j_a?view=graph");
  });
});
