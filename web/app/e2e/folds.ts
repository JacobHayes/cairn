// Folded sections, checked a second time: a screen's collapsed sections (`<details>`) hide content
// the first pass cannot see, so the structural rules run again with every section open. They are
// opened and closed again with no toggle reaching the app, which would keep it as the viewer's
// choice, so the state is as it was after. A tree's or a card's fold hides more rows or cards of
// the kinds the first pass checks.
import type { Page } from "@playwright/test";

import { settled } from "./shell.ts";
import { expectStructure } from "./structure.ts";

/** Marks the sections the check opened, so it closes those and no others. */
const OPENED = "data-walk-opened";

/** Opens or closes every section `selector` finds, holding back the toggle events from the app until each has fired; returns how many it changed. */
async function toggleQuietly(page: Page, selector: string, open: boolean): Promise<number> {
  // eslint-disable-next-line e2e/settled-reads -- the caller has settled the page
  return page.evaluate(
    async ([selector, open, mark]) => {
      const sections = [...document.querySelectorAll<HTMLDetailsElement>(selector)];
      if (sections.length === 0) {
        return 0;
      }
      let seen = 0;
      let done = (): void => undefined;
      const fired = new Promise<void>((resolve) => {
        done = resolve;
      });
      const stop = (event: Event): void => {
        if (event.target instanceof HTMLDetailsElement && sections.includes(event.target)) {
          event.stopImmediatePropagation();
          seen += 1;
          if (seen === sections.length) {
            done();
          }
        }
      };
      document.addEventListener("toggle", stop, true);
      for (const each of sections) {
        each.open = open;
        each.toggleAttribute(mark, open);
      }
      await fired;
      document.removeEventListener("toggle", stop, true);
      return sections.length;
    },
    [selector, open, OPENED] as const,
  );
}

/** Runs the structural rules over the page with every section open, then closes the ones it opened. */
export async function expectSectionsOpenStructure(page: Page, where: string): Promise<void> {
  await settled(page);
  if ((await toggleQuietly(page, "details:not([open])", true)) === 0) {
    return;
  }
  try {
    await expectStructure(page, `${where} with its sections open`);
  } finally {
    await toggleQuietly(page, `details[${OPENED}]`, false);
  }
}
