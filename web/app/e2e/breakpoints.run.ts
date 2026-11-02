// `mise run sweep`: the structural rules (e2e/invariants.ts, `checkpoint`) over every state of the
// screenshot table at each width where a layout starts. A layout overflows at the narrowest width
// it is used for, which is where it starts; the narrowest phone (390) is the screenshot walk's
// phone run, which checks every state there with the same rules. Light only (light and dark are
// held to the same layout by the screenshot walk). A phone's states are visited below the desktop
// threshold and the desktop's from it. The browser step runs it on every push and in the fast
// check (decisions/).
import { expect, test } from "./test.ts";

import { COLUMN_MIN, DESKTOP_MIN } from "../src/shell/breakpoints.ts";
import { checkpoint } from "./invariants.ts";
import { MAX_STATE_VISITS, visitsAt, WALK_VIEWPORTS, walkStates } from "./screenshots.ts";

/** The height every width is swept at. */
const HEIGHT = 900;

/**
 * Where each layout starts: the desktop (`DESKTOP_MIN`), the inspector's column (`COLUMN_MIN`),
 * and the container-query switches those do not reach, the journey's scroll area (560) and the
 * Summary (960). The stylesheets switch only at the widths breakpoints.ts names (its agreement
 * test holds them to it).
 */
const WIDTHS = [560, DESKTOP_MIN, 960, COLUMN_MIN];

const visits = visitsAt([...Object.values(WALK_VIEWPORTS).map((each) => each.width), ...WIDTHS]);
if (visits > MAX_STATE_VISITS) {
  throw new Error(`the screenshot walk and the sweep visit ${String(visits)} states, past their limit of ${String(MAX_STATE_VISITS)}: cut states or widths`);
}

for (const width of WIDTHS) {
  const phone = width < DESKTOP_MIN;
  test.describe(`sweep ${String(width)}px`, () => {
    // A narrow screen is a touch screen: the coarse-pointer rules apply.
    test.use({ viewport: { width, height: HEIGHT }, hasTouch: phone });
    test(`every state at ${String(width)}px`, async ({ page }) => {
      await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" });
      const failed = await walkStates(page, { width, height: HEIGHT }, phone, async (address, state, reach) => {
        await reach();
        await checkpoint(page, `${address} ${state.name} at ${String(width)}px`);
      });
      expect(failed).toEqual([]);
    });
  });
}
