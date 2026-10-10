// The sync chip over a real connection (design 9): what it says while a write is out, while a
// newer revision is being fetched or cannot be, and what it keeps for you when a change did not
// land. The states a browser alone can reach (saved, rejected) are the node detail's specs'.
import { expect, test, type Page } from "@playwright/test";

import { openNode, pin } from "./detail.ts";
import { fresh, goWithin, hold, live, nodeCard, open, openJourney, rename, save, startRename, syncChip } from "./shell.ts";

const title = (page: Page, node: string) => nodeCard(page, node).getByTestId("title");

test("a rejected change waits under needs-you wherever you go; Go to it returns, Discard drops it", async ({ page }) => {
  const panel = await openNode(page, "browser", "j_vendor_eval", "n_final_report");
  await pin(panel, "2026-11-25");
  await expect(panel.getByTestId("date-conflict")).toBeVisible();
  await expect(syncChip(page)).toHaveText("NOT SAVED · 1");
  // Away from the control, it is still counted, and Go to it comes back to it.
  await goWithin(page, "/library?type=routes");
  await expect(syncChip(page)).toHaveText("NOT SAVED · 1");
  await syncChip(page).click();
  const needsYou = page.getByTestId("sync-popover").getByTestId("sync-problem");
  await expect(needsYou).toContainText("Final report");
  await needsYou.getByRole("button", { name: "Go to it" }).click();
  await expect(page.getByTestId("date-conflict")).toBeVisible();
  // Discard from the popover drops it at the control too.
  await syncChip(page).click();
  await page.getByTestId("sync-popover").getByRole("button", { name: "Discard" }).click();
  await expect(page.getByTestId("date-conflict")).toHaveCount(0);
  await expect(syncChip(page)).not.toHaveAttribute("data-state", "not-saved");
});

test("a screen whose file is gone after a deployment asks for a reload, and another screen still opens", async ({ page }) => {
  await open(page, "browser", "/journeys");
  await page.route(/\/(src\/people|assets)\/Entities[.-]/, (route) => route.abort());
  await page.getByRole("navigation", { name: "Screens", exact: true }).getByRole("link", { name: "Entities" }).click();
  await expect(page.getByTestId("load-failure").getByRole("button", { name: "Reload" })).toBeVisible();
  await expect(syncChip(page)).toBeVisible();
  await goWithin(page, "/journeys");
  await expect(page.getByTestId("load-failure")).toHaveCount(0);
});

test("a write in flight, a revision on its way, and one that cannot be fetched", { tag: "@server" }, async ({ context }) => {
  const [one, two] = [await context.newPage(), await context.newPage()];
  // The node renamed is settled not relevant, which the graph hides unless asked.
  const everything = "?detail=all&show=notrelevant%2Cconditional";
  await openJourney(one, "server", "j_hiring", everything);
  await openJourney(two, "server", "j_hiring", everything);
  await live(two);
  // SAVING: only once the write has been out a moment.
  const send = await hold(one, "**/api/journeys/j_hiring/patches");
  const sending = fresh("Close out");
  await startRename(one, "n_offer", sending);
  await save(one, "n_offer");
  await expect(syncChip(one)).toHaveAttribute("data-state", "saving");
  await send();
  await expect(title(one, "n_offer")).toHaveText(sending);
  // UPDATING: the other page was told, and its refetch is slow.
  const fetch = await hold(two, "**/api/journeys/j_hiring/document");
  await rename(one, "n_offer", fresh("Close out"));
  await expect(syncChip(two)).toHaveAttribute("data-state", "updating");
  await fetch();
  await expect(syncChip(two)).toHaveAttribute("data-state", "in-sync");
  // BEHIND: told, and the refetch fails. It still shows the older revision, says which exists, and a click retries.
  await two.route("**/api/journeys/j_hiring/document", (route) => route.abort());
  const shown = Number(await syncChip(two).getAttribute("data-revision"));
  const latest = fresh("Close out");
  await rename(one, "n_offer", latest);
  await expect(syncChip(two)).toHaveText(`BEHIND · REV ${String(shown + 1)}`);
  await two.unroute("**/api/journeys/j_hiring/document");
  await syncChip(two).click();
  await expect(title(two, "n_offer")).toHaveText(latest);
  await expect(syncChip(two)).toHaveAttribute("data-state", "in-sync");
});
