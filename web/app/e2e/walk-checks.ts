// What the screenshot walk checks of each state once its picture is taken and the page has passed
// the invariants (e2e/invariants.ts): each is spread over the walk's four runs so that no run is
// much slower than the others, and each one that changes the page reaches the state again after.
import { expect, type Page } from "@playwright/test";

import { axeProblems } from "./axe.ts";
import { focusRingProblems } from "./focus.ts";
import { expectFoldsOpenStructure } from "./folds.ts";
import { open } from "./shell.ts";
import { themeProblems } from "./themes.ts";

/** What the walk checks in this state: the run it is in, and how to reach it again. */
export interface Walk {
  /** axe runs once per state, in the viewport the state is pictured at (a phone-only state in the phone's run) and in dark. */
  axe: boolean;
  /** The light run reads the state in dark too. */
  darkToo: boolean;
  /** The first state of its address in the desktop's light run gets the Tab walk. */
  tabs: boolean;
  /** Reaches the state from wherever the page is. */
  reach: () => Promise<void>;
}

/** Reaches the state again from Home: reaching the same address from itself would leave the frame as a walk left it. */
async function reachAgain(page: Page, walk: Walk): Promise<void> {
  await open(page, "browser", "/");
  await walk.reach();
}

/** Runs the checks of this walk over `page`, in the state `label`; fails on the first that finds a problem. */
export async function checkState(page: Page, label: string, walk: Walk): Promise<void> {
  if (walk.axe) {
    expect(await axeProblems(page), `${label} axe`).toEqual([]);
  }
  // The theme changes colours and no box.
  if (walk.darkToo) {
    expect(await themeProblems(page), `${label} in dark`).toEqual([]);
  }
  // The Tab walk moves focus and opens tips.
  if (walk.tabs) {
    const ringless = await focusRingProblems(page);
    await reachAgain(page, walk);
    expect(ringless, `${label} focus rings`).toEqual([]);
  }
  await expectFoldsOpenStructure(page, label, () => reachAgain(page, walk));
}
