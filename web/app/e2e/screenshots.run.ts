// `mise run screenshots`: walks the table in e2e/screenshots.ts on the in-browser host and
// saves a picture of every state at a desktop and a phone width, in light and dark, to
// CAIRN_SCREENSHOTS_DIR as `<address>-<state>-<desktop|phone>-<light|dark>.png`, with a
// `screenshots.json` manifest (file, address, state, viewport, theme, url) beside them.
// Each picture waits for its state's ready sign and passes the invariants first
// (e2e/invariants.ts, `capture`). Not part of the ladder: the screen-critique skill and a
// person read the output.
import { mkdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

import { expect, test } from "./test.ts";

import { capture } from "./invariants.ts";
import { FIXTURE_SUBJECT, SCREENS, seedProposals, type State, type Subject } from "./screenshots.ts";
import { open, settled } from "./shell.ts";
import type { AddressName } from "../src/address.ts";

/** The day the fixtures' last scenario patch applies, so the dates the pictures show read as one history. */
const TODAY = "2026-10-26";

const dir = resolve(process.env["CAIRN_SCREENSHOTS_DIR"] ?? "dist/screenshots");

const VIEWPORTS = { desktop: { width: 1440, height: 900 }, phone: { width: 390, height: 844 } } as const;
const THEMES = ["light", "dark"] as const;

/** The proposal addresses need the proposals drafted, which edits the journey: they come last. */
const NEEDS_PROPOSALS: readonly AddressName[] = ["journeyProposal", "journeyProposalNode", "routeProposal", "routeProposalNode"];

for (const [viewport, size] of Object.entries(VIEWPORTS)) {
  // A phone is a touch screen: the coarse-pointer rules (touch-sized controls) apply to what is pictured.
  test.describe(viewport, () => {
    test.use({ viewport: size, hasTouch: viewport === "phone" });
    for (const theme of THEMES) {
      test(`every state at ${viewport} width, ${theme}`, async ({ page }) => {
        mkdirSync(dir, { recursive: true });
        await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
        await page.clock.setFixedTime(new Date(`${TODAY}T12:00:00Z`));
        await open(page, "browser", "/");
        const taken: Record<string, string>[] = [];
        const failed: string[] = [];
        let on: Subject = FIXTURE_SUBJECT;
        const addresses = Object.keys(SCREENS) as AddressName[];
        const order = [...addresses.filter((each) => !NEEDS_PROPOSALS.includes(each)), ...NEEDS_PROPOSALS];
        for (const address of order) {
          if (NEEDS_PROPOSALS.includes(address) && on.proposals === undefined) {
            // The authoring forms are a desktop's: a phone is resized to it while the proposals are drafted.
            await page.setViewportSize(VIEWPORTS.desktop);
            on = { ...on, proposals: await seedProposals(page, on) };
            await page.setViewportSize(size);
          }
          for (const state of SCREENS[address].filter((each: State) => each.only === undefined || each.only === viewport)) {
            // A state that fails is listed with the rest, so one run shows every picture the table cannot take.
            try {
              // The page is kept between states (the in-browser store lives in it), so what the last one left open or focused is closed first.
              await page.keyboard.press("Escape");
              await page.evaluate(() => { (document.activeElement as HTMLElement | null)?.blur(); });
              await state.reach(page, on);
              await expect(state.ready(page, on), `${address} ${state.name} is drawn`).toBeVisible();
              await settled(page);
              const file = `${address}-${state.name}-${viewport}-${theme}.png`;
              await capture(page, join(dir, file));
              taken.push({ file, address, state: state.name, viewport, theme, url: new URL(page.url()).pathname + new URL(page.url()).search });
            } catch (error) {
              failed.push(`${address} ${state.name}: ${error instanceof Error ? error.message.split("\n").slice(0, 6).join(" | ") : String(error)}`);
            }
          }
        }
        writeFileSync(join(dir, `screenshots.${viewport}-${theme}.part.json`), JSON.stringify(taken, null, 2));
        expect(failed).toEqual([]);
      });
    }
  });
}
