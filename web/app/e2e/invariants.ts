// The invariants every page a browser test opened, and every picture taken of one, must hold:
// nothing spills out of its box (e2e/containment.ts), no key is shown where a name belongs
// (e2e/internal-ids.ts), the only tooltip is the app's (e2e/tips.ts), and on a touch screen every
// control has the touch-sized hit area (e2e/touch.ts). `test.ts` runs them
// after every test; a screenshot (a proof's, or the `mise run screenshots` walk's) goes through
// `capture`, which writes the picture only when they pass.
import type { Page } from "@playwright/test";

import { expectContained } from "./containment.ts";
import { expectNoKeysShown } from "./internal-ids.ts";
import { expectTouchTargets } from "./touch.ts";
import { expectNoClosedTipDrawn, expectNoNativeTooltips } from "./tips.ts";

/**
 * The invariants (containment, no internal key shown, no native tooltip, no closed tip drawn, touch-sized controls on a touch screen) on `page` as it is now. They run once at the
 * end of a test body, so a test that walks through several states, each one a test of its own
 * before they were merged, calls this at the ones that are not its end: a state that spills
 * out of its box, or shows a key where a name belongs, fails where it was reached.
 */
export async function checkpoint(page: Page, where = "the page"): Promise<void> {
  await expectContained(page, where);
  await expectNoKeysShown(page);
  await expectNoNativeTooltips(page);
  await expectNoClosedTipDrawn(page);
  await expectTouchTargets(page, where);
}

/** A screenshot of `page` for a proof or a picture, taken only when it passes the invariants. */
export async function capture(page: Page, path: string, options: { fullPage?: boolean } = {}): Promise<void> {
  await checkpoint(page, path);
  await page.screenshot({ path, ...options });
}
