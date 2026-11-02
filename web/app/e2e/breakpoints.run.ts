// `mise run sweep`: the structural rules (e2e/invariants.ts, `checkpoint`) over every state of the
// screenshot table at each width where a layout starts, chosen by rule: each viewport switch the
// CSS's media queries name (read from the stylesheets, so a new one is swept with no edit here: a
// min-width switches at its value, a max-width one pixel above it), and each container-query
// switch no other chosen width reaches (the Summary at 960, the journey's scroll area at 560). A
// layout overflows at the narrowest width it is used for, which is where it starts; the narrowest
// phone (390) is the screenshot walk's phone run, which checks every state there with the same
// rules. Light only (light and dark are held to the same layout by the screenshot walk). A phone's
// states are visited below the desktop threshold and the desktop's from it. The browser step runs
// it on every push and in the fast check (decisions/).
import { expect, test } from "./test.ts";

import { DESKTOP_MIN } from "../src/shell/breakpoints.ts";
import { checkpoint } from "./invariants.ts";
import { walkStates } from "./screenshots.ts";
import { mediaFeatures } from "./stylesheets.ts";

/** The height every width is swept at. */
const HEIGHT = 900;

/** The container-query switches the other widths do not reach: the Summary's (960) and the journey scroll area's (560). */
const FIXED = [560, 960];

/** Every width at which a media query in the app's stylesheets switches: a `min-width` at its value, a `max-width` one above it. */
function thresholds(): number[] {
  return [
    ...new Set(
      mediaFeatures().flatMap(({ feature }) => {
        const match = /^(min|max)-width: (\d+)px$/.exec(feature);
        return match === null ? [] : [Number(match[2]) + (match[1] === "max" ? 1 : 0)];
      }),
    ),
  ];
}

const WIDTHS = [...new Set([...thresholds(), ...FIXED])].sort((a, b) => a - b);

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
