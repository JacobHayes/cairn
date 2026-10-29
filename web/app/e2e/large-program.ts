// The large program fixture (fixtures/large-program): five stages with nested groups and the
// milestones and decisions between them, large enough that its fit is far below the canvas's
// reading zoom. A browser test or a proof imports the route, publishes it and starts a journey.
import { expect, type Page } from "@playwright/test";

import { fixtureRoute, journeyName, publishDraft, startJourney } from "./around.ts";
import { importRoute, open } from "./shell.ts";

/** The route's id, and keys the specs reach. */
export const LARGE = {
  route: "large-program",
  stages: ["n_shape", "n_plan", "n_build", "n_review", "n_wrap_up"],
  build: "n_build",
  partA: "n_build_part_a",
  helpers: "n_build_helpers",
  componentTwo: "n_build_part_a_two",
} as const;

/** Imports the route, publishes it, and starts a journey from it; the journey's id. */
export async function startLargeProgram(page: Page): Promise<string> {
  await importRoute(page, fixtureRoute(LARGE.route));
  await expect(page).toHaveURL(/\/routes\/large-program/);
  await publishDraft(page);
  return startJourney(page, journeyName("Large program"), { route: LARGE.route });
}

/** The journey's graph at its Actions depth, every card reachable. */
export async function openLargeGraph(page: Page, id: string, query = ""): Promise<void> {
  await open(page, `/journeys/${id}?view=graph&depth=actions${query}`);
  await expect(page.getByTestId("canvas")).toBeVisible();
}
