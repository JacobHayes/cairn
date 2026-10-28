// `mise run screenshots`: walks the table in e2e/screenshots.ts on the in-browser host and
// saves a picture of every state at a desktop and a phone width, in light and dark, to
// CAIRN_SCREENSHOTS_DIR as `<address>-<state>-<desktop|phone>-<light|dark>.png`, with a
// `screenshots.json` manifest (file, address, state, viewport, theme, url) beside them.
// Each picture waits for its state's ready sign and passes the invariants first
// (e2e/invariants.ts, `capture`), then the walk's own checks (e2e/walk-checks.ts). The browser
// step runs it too, with no CAIRN_SCREENSHOTS_DIR: the same checks, and no picture, check file or
// manifest, which nothing there reads.
import { mkdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

import { expect, test } from "./test.ts";

import { capture } from "./invariants.ts";
import { SCREENS, walkStates } from "./screenshots.ts";
import { checkState } from "./walk-checks.ts";

/** Where the pictures go; none are taken without it. */
const dir = process.env["CAIRN_SCREENSHOTS_DIR"] === undefined ? undefined : resolve(process.env["CAIRN_SCREENSHOTS_DIR"]);

const VIEWPORTS = { desktop: { width: 1440, height: 900 }, phone: { width: 390, height: 844 } } as const;
const THEMES = ["light", "dark"] as const;

for (const [viewport, size] of Object.entries(VIEWPORTS)) {
  // A phone is a touch screen: the coarse-pointer rules (touch-sized controls) apply to what is pictured.
  test.describe(`screenshots ${viewport}`, () => {
    test.use({ viewport: size, hasTouch: viewport === "phone" });
    for (const theme of THEMES) {
      test(`every state at ${viewport} width, ${theme}`, async ({ page }) => {
        if (dir !== undefined) {
          mkdirSync(dir, { recursive: true });
        }
        await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
        const taken: Record<string, string>[] = [];
        const failed = await walkStates(page, size, viewport === "phone", async (address, state, reach) => {
          await reach();
          const file = `${address}-${state.name}-${viewport}-${theme}.png`;
          await capture(page, join(dir ?? "", file), { structure: true, ...(dir === undefined ? { dry: true } : {}) });
          const url = new URL(page.url());
          await checkState(page, `${address} ${state.name}`, { axe: viewport === (state.only ?? "desktop") && theme === "dark", darkToo: theme === "light", tabs: viewport === "desktop" && theme === "light" && state === SCREENS[address][0], reach });
          taken.push({ file, checks: file.replace(/\.png$/, ".checks.json"), aria: file.replace(/\.png$/, ".aria.yml"), address, state: state.name, viewport, theme, url: url.pathname + url.search });
        });
        if (dir !== undefined) {
          writeFileSync(join(dir, `screenshots.${viewport}-${theme}.part.json`), JSON.stringify(taken, null, 2));
        }
        expect(failed).toEqual([]);
      });
    }
  });
}
