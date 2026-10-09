// The proof's media for the required note (briefs/proof/7.3/prove.sh): a screenshot of each
// state, written to CAIRN_PROOF_OUT. Each step asserts what its picture is meant to show, so a
// picture of the wrong state fails the run. It runs on the in-browser host, seeded on each load.
import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

import { nextItem, openActing } from "../e2e/acting.ts";
import { journeyName, startJourney } from "../e2e/around.ts";
import { flag, openNode, state } from "../e2e/detail.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
const shot = (page: Page, name: string) => page.screenshot({ path: join(out, `${name}.png`) });

test.use({ viewport: { width: 1280, height: 800 } });

test("done asks for the note, completes with it, and goes stale when the note is removed", async ({ page }) => {
  const journey = await startJourney(page, "browser", journeyName("Screening"), { route: "hiring-loop", version: 1 });
  await openActing(page, "browser", journey, "next/list");
  const item = nextItem(page, "n_screen");
  await item.getByRole("button", { name: "Done", exact: true }).click();
  await item.getByLabel("Note").fill("Covered the role, the timeline and the pay band.");
  await shot(page, "1-done-asks-for-the-note");
  await item.getByRole("button", { name: "Add note and mark done" }).click();
  await expect(nextItem(page, "n_screen")).toHaveCount(0);

  const panel = await openNode(page, "browser", journey, "n_screen");
  await expect(state(panel)).toHaveAttribute("data-status", "done");
  await expect(panel.getByTestId("annotation")).toHaveCount(1);
  await shot(page, "2-done-with-its-note");

  await panel.getByTestId("annotation").getByRole("button", { name: "Remove" }).click();
  await expect(flag(panel, "stale")).toBeVisible();
  await expect(panel.getByTestId("stale-reason")).toContainText("missing note");
  await expect(state(panel)).toHaveAttribute("data-status", "done");
  await panel.getByTestId("stale-reason").scrollIntoViewIfNeeded();
  await shot(page, "3-stale-without-the-note");
});
