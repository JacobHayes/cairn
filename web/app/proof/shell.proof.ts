// The proof's media for brief 4.6 (briefs/proof/4.6/prove.sh): a screenshot of each acceptance
// state of the shell and a short video of its main flow, written to CAIRN_PROOF_OUT. Each step also
// asserts what the picture is meant to show, so a picture of the wrong state fails the run.
// The proof run starts its own fixture server, so its titles need not be unique.
import { join } from "node:path";

import { expect, test, type Browser, type Page } from "@playwright/test";

import { nodeCard, open, openJourney, rename, renameOf, save, startRename } from "../e2e/shell.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
const shot = (page: Page, name: string) => page.screenshot({ path: join(out, `${name}.png`) });
/** A beat for the video, so a viewer can follow each step. */
const beat = (page: Page) => page.waitForTimeout(700);

test("the journey index and a journey derived in the worker, on the in-browser host", async ({ page }) => {
  await open(page, "browser");
  await expect(page.getByTestId("journey-row")).toHaveCount(4);
  await shot(page, "1-index-in-browser");
  await page.getByRole("link", { name: "Launch the reporting release" }).click();
  await expect(page.getByTestId("derivation")).toBeVisible();
  await shot(page, "2-journey-derived-in-worker");
});

test("a live update after a patch in another tab, on the server host", async ({ context }) => {
  const [editor, other] = [await context.newPage(), await context.newPage()];
  await openJourney(editor, "server", "j_vendor_eval");
  await openJourney(other, "server", "j_vendor_eval");
  await expect(other.getByTestId("live")).toHaveAttribute("data-status", "live");
  const renamed = "Findings writeup, revised";
  await rename(editor, "n_findings", renamed);
  await expect(editor.getByTestId("notice")).toHaveAttribute("data-tone", "saved");
  await shot(editor, "3a-patch-saved-with-consequences");
  await expect(nodeCard(other, "n_findings").getByTestId("title")).toHaveText(renamed);
  await shot(other, "3b-other-tab-updated-live");
});

test("a conflict on one field, surfaced with what intervened", async ({ context }) => {
  const [one, two] = [await context.newPage(), await context.newPage()];
  await openJourney(one, "server", "j_vendor_eval");
  await openJourney(two, "server", "j_vendor_eval");
  await startRename(two, "n_plan", "Test plan, mine");
  await rename(one, "n_plan", "Test plan, theirs");
  await save(two, "n_plan");
  await expect(two.getByTestId("conflict")).toBeVisible();
  await shot(two, "4-conflict-surfaced");
});

test("version skew stops the tab and asks for a reload; the draft survives it", async ({ context }) => {
  const [one, two] = [await context.newPage(), await context.newPage()];
  await openJourney(one, "server", "j_bakeoff");
  await openJourney(two, "server", "j_bakeoff");
  const unsent = "Recommendation summary, unsent";
  await startRename(two, "n_summary", unsent);
  await two.route("**/api/journeys/j_bakeoff/document", async (route) => {
    const response = await route.fetch();
    const document = (await response.json()) as { engine_version: string };
    await route.fulfill({ response, json: { ...document, engine_version: "99.0.0" } });
  });
  await rename(one, "n_comparison", "Side-by-side comparison, final");
  await expect(two.getByTestId("skew")).toBeVisible();
  await shot(two, "5-version-skew-banner");
  await two.unroute("**/api/journeys/j_bakeoff/document");
  await two.getByRole("button", { name: "Reload" }).click();
  await expect(renameOf(two, "n_summary").getByRole("textbox")).toHaveValue(unsent);
  await shot(two, "6-draft-survives-reload");
});

/** The main flow, recorded: index, a journey, an edit with its notice, another tab's edit arriving. */
async function mainFlow(browser: Browser, baseURL: string): Promise<void> {
  const context = await browser.newContext({
    baseURL,
    viewport: { width: 960, height: 600 },
    recordVideo: { dir: join(out, "video"), size: { width: 960, height: 600 } },
  });
  const page = await context.newPage();
  const other = await context.newPage();
  await open(page, "server");
  await beat(page);
  await page.getByRole("link", { name: "Hire a platform engineer" }).click();
  await expect(page.getByTestId("derivation")).toBeVisible();
  await beat(page);
  await startRename(page, "n_offer", "Offer letter, signed");
  await beat(page);
  await save(page, "n_offer");
  await expect(page.getByTestId("notice")).toHaveAttribute("data-tone", "saved");
  await beat(page);
  await openJourney(other, "server", "j_hiring");
  const theirs = "Close out with the candidate, by phone";
  await rename(other, "n_close_out", theirs);
  await expect(nodeCard(page, "n_close_out").getByTestId("title")).toHaveText(theirs);
  await beat(page);
  await beat(page);
  await other.close();
  await context.close();
  await page.video()?.saveAs(join(out, "main-flow.webm"));
}

test("the main flow, on video", async ({ browser, baseURL }) => {
  await mainFlow(browser, baseURL ?? "");
});
