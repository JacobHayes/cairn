// What the screenshot walk checks of each state once its picture is taken and the page has passed
// the invariants (e2e/invariants.ts): each is spread over the walk's runs so that no run is much
// slower than the others, and each leaves the state as it found it.
import { expect, type Page } from "@playwright/test";

import { axeProblems } from "./axe.ts";
import { expectSectionsOpenStructure } from "./folds.ts";
import { themeProblems } from "./themes.ts";

/** What the walk checks in this state, by the run it is in. */
export interface Walk {
  /** axe runs once per state, in the light run of the viewport the state is pictured at (a phone-only state in the phone's). */
  axe: boolean;
  /** The light run reads the state in dark too. */
  darkToo: boolean;
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
  await expectSectionsOpenStructure(page, label);
}
