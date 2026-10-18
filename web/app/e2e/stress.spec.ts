// The containment invariant over worst-case content (e2e/stress.ts): the stress journey walked
// through every screen and width in both themes. Each step ends in a check (e2e/test.ts runs the
// invariant after the test; `expectContained` runs it between the steps, so a failure names the
// screen).
import { expect, test } from "./test.ts";

import { expectTitleLinesAsLaidOut, viewportOf, viewportSettled } from "./canvas.ts";
import { NEAR_ZOOM } from "../src/canvas/gestures.ts";
import { containmentProblems } from "./containment.ts";
import { nodeCard, open, openAt } from "./shell.ts";
import { STRESS, publishStressJourney } from "./stress.ts";

for (const scheme of ["light", "dark"] as const) {
  test(`the stress journey stays inside its boxes on every screen (${scheme})`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: scheme });
    const journey = await publishStressJourney(page);
    // Every screen is walked and every problem listed with the screen it is on, so one run shows them all.
    const found: string[] = [];
    const look = async (where: string) => {
      found.push(...(await containmentProblems(page)).map((problem) => `${where}: ${problem}`));
    };
    const check = async (address: string, node?: string) => {
      await openAt(page, "browser", journey, address, node === undefined ? {} : { node });
      await look(`${address}${node === undefined ? "" : ` with ${node}`} at ${String(page.viewportSize()?.width ?? 0)}px`);
    };
    await check("graph");
    await expect(page.getByTestId("node-card").first()).toBeVisible();
    await page.keyboard.press("f");
    await viewportSettled(page);
    await look("graph fitted");
    // A window that holds the whole graph at its reading size, so every card is drawn at the near zoom.
    await page.setViewportSize({ width: 4200, height: 3200 });
    await check("graph");
    await page.keyboard.press("f");
    await viewportSettled(page);
    expect((await viewportOf(page)).zoom).toBeGreaterThanOrEqual(NEAR_ZOOM);
    await look("graph at the reading zoom");
    await expectTitleLinesAsLaidOut(page);
    // Folded, a stage is a card of the usual width: its head holds the kind, CURRENT, the chip and the control.
    for (const stage of STRESS.stages) {
      const fold = nodeCard(page, stage).getByTestId("card-collapse");
      if ((await fold.count()) > 0) {
        await fold.click();
      }
    }
    await page.keyboard.press("f");
    await viewportSettled(page);
    await look("graph with the stages folded");
    await page.setViewportSize({ width: 1280, height: 720 });
    await check("list");
    await check("timeline");
    await check("summary");
    await check("list", "n_a1");
    // Every inspector section open, then the popovers and a card's menu, each holding its own content.
    await page.evaluate(() => { document.querySelectorAll<HTMLDetailsElement>(".inspector details").forEach((section) => { section.open = true; }); });
    await look("the inspector with every section open");
    for (const trigger of ["filter-button", "depth-button", "view-picker", "sync"]) {
      await page.getByTestId(trigger).click();
      await look(`${trigger} open`);
      await page.keyboard.press("Escape");
    }
    await check("graph");
    await nodeCard(page, "n_a1").click({ button: "right" });
    await look("a card's menu");
    await page.keyboard.press("Escape");
    await check("walk");
    // The screens that list journeys, routes and people, holding the stress journey's and entity's names.
    const index = async (path: string) => {
      await open(page, "browser", path);
      await look(`${path} at ${String(page.viewportSize()?.width ?? 0)}px`);
    };
    for (const width of [1280, 390]) {
      await page.setViewportSize({ width, height: width === 390 ? 844 : 720 });
      for (const address of ["graph", "list", "timeline", "summary", "walk"]) {
        await check(address);
      }
      for (const path of ["/", "/journeys", "/routes", "/entities", "/me"]) {
        await index(path);
      }
    }
    expect(found).toEqual([]);
  });
}
