// The proof's pictures for brief 5.5 (briefs/proof/5.5/prove.sh): a screenshot of each
// acceptance state of the journey index and overview, the cross-journey "mine" list, journey
// creation, the route screens, entities, and identity, a short video of the main flow, and
// the values the README tabulates, written to CAIRN_PROOF_OUT. Each step asserts what its
// picture is meant to show, so a picture of the wrong state fails the run. Everything runs on
// the in-browser host, seeded on each load, except the linked identity, on the server host.
import { writeFileSync } from "node:fs";
import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

import {
  addEntity,
  fixtureRoute,
  mergeEntities,
  openJourneyCard,
  openRouteDetail,
  routeAction,
  savedText,
  setStatus,
  startJourney,
  versionJourneys,
} from "../e2e/around.ts";
import { section } from "../e2e/detail.ts";
import { fresh, nodeCard, open, openFromCanvas, openJourney } from "../e2e/shell.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
const shot = (page: Page, name: string) => page.screenshot({ path: join(out, `${name}.png`), fullPage: true });
const beat = (page: Page) => page.waitForTimeout(700);
const values: Record<string, unknown> = {};
const record = (name: string, value: unknown) => {
  values[name] = value;
  writeFileSync(join(out, "around.json"), JSON.stringify(values, null, 2));
};

test.use({ viewport: { width: 1200, height: 900 } });
test.describe.configure({ mode: "serial" });

test("C17: a route with three versions, journeys on each, retired and still offering upgrades", async ({ page }) => {
  await open(page, "browser", "/library");
  await expect(page.getByTestId("route-row").first()).toBeVisible();
  await shot(page, "1-route-index");
  await openRouteDetail(page, "browser", "vendor-evaluation");
  for (const version of [2, 3]) {
    await routeAction(page, "Open a draft");
    await routeAction(page, "Publish the draft");
    await startJourney(page, "browser", `On version ${String(version)}`, { route: "vendor-evaluation", version });
    await openRouteDetail(page, "browser", "vendor-evaluation");
  }
  await routeAction(page, "Retire");
  const marks = { 1: await versionJourneys(page, 1), 2: await versionJourneys(page, 2), 3: await versionJourneys(page, 3) };
  expect(Object.values(marks[3])).toEqual(["none"]);
  record("routeDetail", marks);
  await shot(page, "2-route-detail-three-versions-retired");
  await open(page, "browser", "/journeys?status=any&route=vendor-evaluation&upgrade=1");
  await expect(page.getByTestId("journey-row")).toHaveCount(2);
  await shot(page, "3-index-filtered-upgrade-available");
});

test("A13, A11: the fixture's file imported and exported identically; a second import while the draft is open", async ({ page }) => {
  await openRouteDetail(page, "browser", "hiring-loop");
  const input = page.getByTestId("route-actions").getByLabel("Import a file as a new draft");
  await input.setInputFiles(fixtureRoute("hiring-loop"));
  await expect(page.getByTestId("draft")).toHaveAttribute("data-status", "open");
  const draft = await savedText(page, () => page.getByTestId("route-actions").getByRole("button", { name: "Export the draft" }).click());
  const version = await savedText(page, () => page.locator('[data-testid="version"][data-version="1"]').getByRole("button", { name: "Export" }).click());
  expect(draft).toBe(version);
  record("export", { draftBytes: draft.length, versionBytes: version.length, identical: draft === version });
  await shot(page, "4-imported-as-a-draft");
  await input.setInputFiles(fixtureRoute("hiring-loop"));
  await expect(page.locator('[data-testid="violation"][data-code="draft_exists"]')).toBeVisible();
  await shot(page, "5-import-refused-while-a-draft-is-open");
});

test("C16: the index, mine across journeys, a new journey, and its overview", async ({ page }) => {
  await open(page, "browser", "/journeys?status=any");
  await expect(page.getByTestId("journey-row").first()).toBeVisible();
  await shot(page, "6-journey-index");
  await open(page, "browser", "/mine");
  await expect(page.getByTestId("mine-journey").first()).toBeVisible();
  record("mine", await page.getByTestId("mine-journey").evaluateAll((items) => items.map((item) => [item.getAttribute("data-journey"), item.querySelectorAll("[data-testid=mine-item]").length])));
  await shot(page, "7-mine-across-journeys");
  await open(page, "browser", "/new?route=hiring-loop");
  await page.getByTestId("new-journey-form").getByLabel("Name").fill("Hire a data engineer");
  await shot(page, "8-new-journey-from-a-route");
  await page.getByRole("button", { name: "Start the journey" }).click();
  await expect(page.getByTestId("triage-card")).toBeVisible();
  await shot(page, "9-lands-in-the-walkthrough");
  const empty = await startJourney(page, "browser", "Plan the offsite");
  await openJourneyCard(page, "browser", empty);
  await expect(page.getByTestId("completion-suggested")).toBeVisible();
  await openJourneyCard(page, "browser", "j_vendor_eval");
  await shot(page, "10-overview");
  await setStatus(page, "completed");
  await setStatus(page, "archived");
  await page.getByTestId("delete-journey").getByRole("textbox").fill((await page.getByTestId("journey-name").textContent()) ?? "");
  await expect(page.getByTestId("delete-journey").getByRole("button", { name: "Delete the journey" })).toBeEnabled();
  await shot(page, "11-archived-with-delete-behind-its-name");
});

test("E6: two entities merged with history intact", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval");
  const held = await section(await openFromCanvas(page, "n_who_owns"), "history");
  await expect(held.getByTestId("history-patch").first()).toBeVisible();
  const before = await held.getByTestId("history-patch").count();
  await open(page, "browser", "/entities");
  const director = await addEntity(page, "Evaluation Director");
  await mergeEntities(page, director, "e_lead");
  await shot(page, "12-entities-merged");
  await openJourney(page, "browser", "j_vendor_eval");
  await expect(nodeCard(page, "n_plan").getByTestId("card-owner")).toContainText("Evaluation Director");
  const history = await section(await openFromCanvas(page, "n_who_owns"), "history");
  await expect(history.getByTestId("history-patch")).toHaveCount(before);
  record("merge", { survivor: director, merged: "e_lead", historyBefore: before, historyAfter: await history.getByTestId("history-patch").count() });
  await shot(page, "13-the-journey-reads-the-survivor");
});

test("H3: the local identity's duplicates offered for merging, and an identity linked on the server", async ({ page }) => {
  await open(page, "browser", "/me");
  await expect(page.getByTestId("merge-offer")).toBeVisible();
  await shot(page, "14-identity-in-browser");
  const email = `${fresh("linked").replace(/\W/g, "-")}@example.org`;
  await open(page, "server", "/entities");
  const entity = await addEntity(page, "Linked Person", email);
  await open(page, "server", "/me");
  await page.getByRole("link", { name: "Sign in with stub" }).click();
  await page.getByLabel("Subject").fill(fresh("subject").replace(/\W/g, "-"));
  await page.getByLabel("Name").fill("Linked Person");
  await page.getByLabel("Email", { exact: true }).fill(email);
  await page.getByRole("button", { name: "Sign in" }).click();
  await expect(page.locator(`[data-testid="your-entity"][data-entity="${entity}"]`)).toBeVisible();
  record("linked", { email, entity, providers: await page.getByTestId("identity").evaluateAll((rows) => rows.map((row) => row.getAttribute("data-provider"))) });
  await shot(page, "15-identity-linked-on-the-server");
});

test("the dark theme and a narrow screen", async ({ browser, baseURL }) => {
  const dark = await browser.newPage({ baseURL: baseURL ?? "", colorScheme: "dark", viewport: { width: 1200, height: 900 } });
  await openRouteDetail(dark, "browser", "vendor-evaluation");
  await shot(dark, "16-dark-theme");
  await dark.close();
  const narrow = await browser.newPage({ baseURL: baseURL ?? "", viewport: { width: 390, height: 844 } });
  await open(narrow, "browser", "/journeys?status=any");
  await expect(narrow.getByTestId("journey-row").first()).toBeVisible();
  expect(await narrow.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  await shot(narrow, "17-narrow-screen");
  await narrow.close();
});

test("the main flow, on video", async ({ browser, baseURL }) => {
  const context = await browser.newContext({ baseURL: baseURL ?? "", viewport: { width: 1100, height: 680 }, recordVideo: { dir: join(out, "video"), size: { width: 1100, height: 680 } } });
  const page = await context.newPage();
  await open(page, "browser", "/journeys");
  await beat(page);
  await page.getByTestId("new-journey").click();
  await page.getByTestId("new-journey-form").getByLabel("Name").fill("Evaluate a second vendor");
  await page.getByLabel("Start from").selectOption("vendor-evaluation");
  await beat(page);
  await page.getByRole("button", { name: "Start the journey" }).click();
  await expect(page.getByTestId("triage-card")).toBeVisible();
  await beat(page);
  await expect(page.getByTestId("journey-card")).toBeVisible();
  await beat(page);
  await setStatus(page, "completed");
  await beat(page);
  await setStatus(page, "archived");
  await beat(page);
  await page.getByRole("link", { name: "Routes" }).click();
  await page.getByRole("link", { name: "Vendor evaluation" }).click();
  await expect(page.getByTestId("route-detail")).toBeVisible();
  await beat(page);
  await context.close();
  await page.video()?.saveAs(join(out, "main-flow.webm"));
});
