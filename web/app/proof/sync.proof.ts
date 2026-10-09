// The proof's media for the sync chip (briefs/proof/8.5/prove.sh): a picture of the strip's
// right end in each state the chip can say, each reached the way a person would reach it
// (a slowed or refused request, a second tab, a rejected change), and the receipt under a
// control. Each step asserts the state its picture shows. CAIRN_PROOF_OUT names where they go.
import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

import { openNode, pin } from "../e2e/detail.ts";
import { fresh, goWithin, hold, live, nodeCard, open, openJourney, recentSaves, rename, renameOf, save, startRename, syncChip } from "../e2e/shell.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
const SIZE = { width: 1100, height: 600 };
test.use({ viewport: SIZE });

/** The strip's right end and what rises from it. */
const shot = (page: Page, name: string, height = 150) =>
  page.screenshot({ path: join(out, `${name}.png`), clip: { x: SIZE.width - 500, y: SIZE.height - height, width: 500, height } });

const state = (page: Page, value: string) => expect(syncChip(page)).toHaveAttribute("data-state", value);

test("saved, the receipt with its warning, the popover, rejected, and the demo", async ({ page }) => {
  await open(page, "browser", "/");
  await state(page, "demo");
  await shot(page, "demo");
  const offer = await openNode(page, "browser", "j_hiring", "n_make_offer");
  await offer.getByTestId("actions").getByRole("button", { name: "Reopen" }).click();
  const panel = await openNode(page, "browser", "j_hiring", "n_close_out");
  await panel.getByTestId("actions").getByRole("button", { name: "Complete" }).click();
  await expect(panel.getByTestId("actions").getByTestId("receipt-warning")).toBeVisible();
  await state(page, "saved");
  await page.screenshot({ path: join(out, "receipt.png") });
  await shot(page, "saved");
  await recentSaves(page);
  await syncChip(page).click();
  await expect(page.getByTestId("sync-recent").first()).toBeVisible();
  await shot(page, "popover", 340);
  await page.keyboard.press("Escape");
  const report = await openNode(page, "browser", "j_vendor_eval", "n_final_report");
  await pin(report, "2026-11-25");
  await expect(report.getByTestId("date-conflict")).toBeVisible();
  await state(page, "not-saved");
  await syncChip(page).click();
  await expect(page.getByTestId("sync-problem")).toHaveCount(1);
  await shot(page, "not-saved", 340);
});

test("the states over a connection", { tag: "@server" }, async ({ context }) => {
  const [one, two] = [await context.newPage(), await context.newPage()];
  await openJourney(one, "server", "j_hiring");
  await openJourney(two, "server", "j_hiring");
  await live(two);
  await shot(two, "in-sync");
  const send = await hold(one, "**/api/journeys/j_hiring/patches");
  await startRename(one, "n_close_out", fresh("Close out"));
  await save(one, "n_close_out");
  await state(one, "saving");
  await shot(one, "saving");
  const fetch = await hold(two, "**/api/journeys/j_hiring/document");
  await send();
  await state(two, "updating");
  await shot(two, "updating");
  await fetch();
  await state(two, "in-sync");
  await two.route("**/api/journeys/j_hiring/document", (route) => route.abort());
  await rename(one, "n_close_out", fresh("Close out"));
  await expect(syncChip(two)).toHaveText(/^BEHIND · REV \d+$/);
  await shot(two, "behind");
  await two.unroute("**/api/journeys/j_hiring/document");
  await syncChip(two).click();
  await state(two, "in-sync");
  // A rename in each tab over the same title: the second is a conflict.
  const [theirs, mine] = [fresh("Theirs"), fresh("Mine")];
  await startRename(two, "n_onsite", mine);
  await rename(one, "n_onsite", theirs);
  await save(two, "n_onsite");
  await expect(renameOf(two, "n_onsite")).toBeVisible();
  await state(two, "conflict");
  await shot(two, "conflict");
  await two.getByRole("button", { name: "Keep my edit on the current version" }).click();
  await save(two, "n_onsite");
  await expect(nodeCard(two, "n_onsite").getByTestId("title")).toHaveText(mine);
  await two.route("**/api/events/stream*", (route) => route.abort());
  await goWithin(two, "/journeys/j_vendor_eval");
  await state(two, "reconnecting");
  await shot(two, "reconnecting");
  await two.unroute("**/api/events/stream*");
  await live(two);
  await context.setOffline(true);
  await state(two, "offline");
  await shot(two, "offline");
  await context.setOffline(false);
  await two.route("**/api/journeys/j_hiring/document", async (route) => {
    const response = await route.fetch();
    const document = (await response.json()) as { engine_version: string };
    await route.fulfill({ response, json: { ...document, engine_version: "99.0.0" } });
  });
  await goWithin(two, "/journeys/j_hiring");
  await rename(one, "n_offer", fresh("Offer"));
  await state(two, "new-version");
  await shot(two, "new-version");
});
