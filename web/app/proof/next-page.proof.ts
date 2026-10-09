// The proof's media for brief 8.9 (briefs/proof/8.9/prove.sh): a screenshot of each state of
// the Next page (the list, its folds, the line for when nothing needs you, the stalled
// diagnostic), the cards (a card with the pass rail, a node opened from it, the end of a pass,
// the decision walkthrough) and Mine, and a short video of a walkthrough pass, written to
// CAIRN_PROOF_OUT. It navigates and captures only. The walkthrough runs
// on the server host over a journey started from the vendor evaluation's route; everything
// else on the in-browser host, seeded on each load.
import { join } from "node:path";

import { test, type Browser, type Page } from "@playwright/test";

import { answerCard, card, daysAfter, nextItem, openActing, passOrder, startVendorJourney } from "../e2e/acting.ts";
import { journeyName, startJourney } from "../e2e/around.ts";
import { openNode, section } from "../e2e/detail.ts";
import { goWithin, nodePanel, syncChip } from "../e2e/shell.ts";
import { FIXED_TODAY } from "../e2e/views.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
const shot = (page: Page, name: string) => page.screenshot({ path: join(out, `${name}.png`) });
const beat = (page: Page) => page.waitForTimeout(700);

test.use({ viewport: { width: 1200, height: 760 } });
test.describe.configure({ mode: "serial" });

async function atFixedToday(page: Page): Promise<void> {
  await page.clock.setFixedTime(new Date(`${FIXED_TODAY}T12:00:00Z`));
}

test("the list, hovering a row, and the folds", async ({ page }) => {
  await atFixedToday(page);
  await openActing(page, "browser", "j_launch", "next/list");
  await page.getByTestId("rank-tag").first().waitFor();
  await nextItem(page, "n_docs").hover();
  await shot(page, "1-the-next-list");
  const group = await openNode(page, "browser", "j_launch", "n_materials");
  const blocking = await section(group, "blocking");
  const today = (await syncChip(page).getAttribute("data-today")) ?? "";
  await blocking.getByRole("button", { name: "Snooze until a date" }).click();
  await blocking.getByLabel("Snooze until").fill(daysAfter(today, 7));
  await blocking.getByTestId("snooze").getByRole("button", { name: "Save" }).click();
  await blocking.getByTestId("snooze").getByText("Snoozed until").waitFor();
  await openActing(page, "browser", "j_launch", "next/list");
  await page.getByTestId("fold-look").locator("summary").click();
  await page.getByTestId("fold-snoozed").locator("summary").click();
  await page.getByTestId("snooze-group").first().waitFor();
  await shot(page, "2-the-folds-open");
});

test("nothing needs you now, and the stalled journey", async ({ page }) => {
  await openActing(page, "browser", "j_bakeoff", "next/list");
  await page.getByTestId("next-for-you").waitFor();
  await shot(page, "3-nothing-needs-you-now");
  await openActing(page, "browser", "j_hiring", "next/list");
  const today = (await syncChip(page).getAttribute("data-today")) ?? "";
  await nextItem(page, "n_offer").getByRole("link", { name: "Offer letter" }).click();
  const blocking = await section(nodePanel(page, "n_offer"), "blocking");
  await blocking.getByRole("button", { name: "Snooze until a date" }).click();
  await blocking.getByLabel("Snooze until").fill(daysAfter(today, 7));
  await blocking.getByTestId("snooze-date-form").getByRole("button", { name: "Save" }).click();
  await page.getByTestId("stalled").waitFor();
  await shot(page, "4-stalled-with-unsnooze");
});

test("the cards, the pass rail, a node opened from it, and the end of a pass", async ({ page }) => {
  await atFixedToday(page);
  await openActing(page, "browser", "j_launch", "next/cards");
  await card(page).waitFor();
  await shot(page, "5-a-card-and-the-pass-rail");
  await page.getByTestId("pass-rail").getByRole("link", { name: "Documentation" }).click();
  await page.getByTestId("back-to-pass").waitFor();
  await shot(page, "6-a-node-opened-from-the-pass");
  await page.getByTestId("back-to-pass").click();
  for (let at = 0; at < 5; at += 1) {
    await card(page).getByTestId("pass").click();
  }
  await page.getByTestId("pass-done").waitFor();
  await shot(page, "7-every-card-seen-once");
});

test("the walkthrough on start", async ({ page }) => {
  const journey = await startVendorJourney(page);
  await openActing(page, "server", journey, "next/cards?decisions=1");
  await page.getByTestId("walkthrough-intro").waitFor();
  await shot(page, "8-the-walkthrough-on-start");
});

test("Mine", async ({ page }) => {
  const journey = await startJourney(page, "browser", journeyName("Vendor evaluation"), { route: "vendor-evaluation", version: 1 });
  await openActing(page, "browser", journey, "next/list");
  await nextItem(page, "n_partner_runs").getByLabel("Assign owner").selectOption("e_lead");
  await nextItem(page, "n_partner_runs").getByTestId("receipt").waitFor();
  await goWithin(page, "/mine");
  await page.getByTestId("mine-journey").first().waitFor();
  await shot(page, "9-mine");
});

async function walkthroughPass(browser: Browser, baseURL: string): Promise<void> {
  const context = await browser.newContext({
    baseURL,
    viewport: { width: 1100, height: 700 },
    recordVideo: { dir: join(out, "video"), size: { width: 1100, height: 700 } },
  });
  const page = await context.newPage();
  const journey = await startVendorJourney(page);
  await openActing(page, "server", journey, "next/cards?decisions=1");
  await page.getByTestId("walkthrough-intro").waitFor();
  await beat(page);
  await answerCard(page, "yes");
  await beat(page);
  const first = (await passOrder(page))[0] ?? "";
  await page.keyboard.press("p");
  await page.waitForFunction((was) => document.querySelector('[data-testid="triage-card"]')?.getAttribute("data-node") !== was, first);
  await beat(page);
  await page.getByTestId("pass-rail").getByRole("link").first().click();
  await page.getByTestId("back-to-pass").waitFor();
  await beat(page);
  await page.getByTestId("back-to-pass").click();
  await beat(page);
  await context.close();
  await page.video()?.saveAs(join(out, "a-walkthrough-pass.webm"));
}

test("a walkthrough pass, on video", async ({ browser, baseURL }) => {
  await walkthroughPass(browser, baseURL ?? "");
});
