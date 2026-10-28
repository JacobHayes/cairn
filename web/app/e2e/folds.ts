// Folded content, checked a second time: a screen's folds (a `<details>`, a tree's `.tree-fold`,
// a collapsed canvas card's expand button) hide content the first pass cannot see, so the
// structural rules run again with every fold open. Opening changes the page, so the caller puts
// the state back by reaching it again. Opening a section would also be remembered as the
// viewer's choice (the inspector keeps its sections' folds), so `openFolds` keeps the toggle
// events from reaching the app until `closeFolds`.
import type { Page } from "@playwright/test";

import { whenSettled } from "./shell.ts";

/** What opens a fold, in the page. A tree or card fold can reveal more folds, so the walk repeats. */
const CLOSED = "details:not([open]), button.tree-fold[aria-expanded='false'], [data-testid='card-expand']";

/** More passes than any screen has folds nested: a pass that opens none ends the walk, so only a fold that keeps reappearing runs into it. */
const PASSES_MAX = 100;

const STOP = "__foldsStop";

/** Opens every fold the page shows, and those they reveal; returns how many it opened. */
export async function openFolds(page: Page): Promise<number> {
   
  await page.evaluate((name) => {
    const stop = (event: Event): void => { event.stopImmediatePropagation(); };
    Reflect.set(window, name, stop);
    document.addEventListener("toggle", stop, true);
  }, STOP);
  let opened = 0;
  for (let pass = 0; pass < PASSES_MAX; pass += 1) {
    // Clicks in one turn can overwrite each other's state, so a fold may stay closed for the next pass.
     
    const now = await whenSettled(page, () =>
      page.evaluate((selector) => {
        const closed = [...document.querySelectorAll<HTMLElement>(selector)];
        for (const each of closed) {
          if (each instanceof HTMLDetailsElement) {
            each.open = true;
          } else {
            each.click();
          }
        }
        return closed.length;
      }, CLOSED),
    );
    if (now === 0) {
      return opened;
    }
    opened += now;
  }
  throw new Error(`folds were still closed after ${String(PASSES_MAX)} passes`);
}

/** Lets toggle events reach the app again. */
export async function closeFolds(page: Page): Promise<void> {
   
  await page.evaluate((name) => {
    const stop = Reflect.get(window, name) as EventListener | undefined;
    if (stop !== undefined) {
      document.removeEventListener("toggle", stop, true);
    }
  }, STOP);
}
