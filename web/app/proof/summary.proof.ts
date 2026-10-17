// The proof's media for the Summary (briefs/proof/2026-10-17-journey-summary/prove.sh): the
// journey's front page on the fixtures at the day each scenario has reached, in its distinct
// states (mid-journey, with trouble, fresh, stalled, completed, archived, empty, on a phone, in
// the dark theme, in print) and a short video of the main flow, written to CAIRN_PROOF_OUT.
// Steps only wait for the state they picture; the e2e specs assert it. Everything runs on the
// in-browser host.
//
// usage: (cd web/app && CAIRN_PROOF_OUT=dist/proof npx playwright test -c playwright.proof.config.ts proof/summary.proof.ts)
import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

import { addEntity, setStatus, startJourney } from "../e2e/around.ts";
import { goWithin, importRoute, open, openAt } from "../e2e/shell.ts";
import { rowsOf, stall, startOnSummary } from "../e2e/summary.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
const beat = (page: Page, ms = 900) => page.waitForTimeout(ms);
const shot = (page: Page, name: string, fullPage = false) => page.screenshot({ path: join(out, `${name}.png`), fullPage });

// A day on which each fixture's scenario has happened, so what finished lately is in the past.
const VENDOR_DAY = "2026-10-27";
const LAUNCH_DAY = "2026-11-05";
const BAKEOFF_DAY = "2026-10-07";

test.use({ viewport: { width: 1400, height: 900 }, colorScheme: "light" });

/** The whole page in one frame: a tall window, not a full-page capture, which would drop the touch media of an emulated phone. */
async function tall(page: Page, height = 2700): Promise<void> {
  await page.setViewportSize({ width: 390, height });
  await beat(page, 600);
}

async function ready(page: Page): Promise<void> {
  await expect(page.getByTestId("summary-lead")).toBeVisible();
  await beat(page, 500);
}

test("mid-journey, with trouble, and in the dark", async ({ page }) => {
  await openAt(page, "browser", "j_vendor_eval", "summary", { fixedToday: VENDOR_DAY });
  await ready(page);
  await shot(page, "1-mid-journey");
  await page.emulateMedia({ colorScheme: "dark" });
  await beat(page, 400);
  await shot(page, "9-dark");
  await page.emulateMedia({ colorScheme: "light" });

  await openAt(page, "browser", "j_launch", "summary", { fixedToday: LAUNCH_DAY });
  await ready(page);
  await shot(page, "2-trouble");
  await page.emulateMedia({ media: "print", colorScheme: "light" });
  await beat(page, 400);
  await shot(page, "10-print", true);
});

test("a fresh journey", async ({ page }) => {
  await page.clock.setFixedTime(new Date("2026-10-01T12:00:00Z"));
  await startOnSummary(page, "vendor-evaluation", "Evaluate a reporting vendor");
  await ready(page);
  await shot(page, "3-fresh");
});

test("stalled, completed and archived", async ({ page }) => {
  await stall(page, "j_bakeoff", ["n_trial_a", "n_trial_b", "n_wrap_up"]);
  await ready(page);
  await shot(page, "4-stalled");
});

test("completed", async ({ page }) => {
  await openAt(page, "browser", "j_bakeoff", "summary", { fixedToday: BAKEOFF_DAY });
  await setStatus(page, "completed");
  await ready(page);
  // The save's receipt clears itself; the picture is the page at rest.
  await expect(page.getByTestId("receipt")).toHaveCount(0, { timeout: 10000 });
  await shot(page, "5-completed");
});

test("archived with a note", async ({ page }) => {
  await openAt(page, "browser", "j_bakeoff", "summary", { fixedToday: BAKEOFF_DAY });
  const about = page.getByTestId("summary-about");
  await about.getByRole("button", { name: "Add a note or link" }).click();
  await about.getByLabel("Note").fill("Option B needs a licence check before the trial.");
  await about.getByRole("button", { name: "Add", exact: true }).click();
  await expect(about.getByTestId("annotation")).toHaveCount(1);
  await setStatus(page, "archived");
  await ready(page);
  await expect(page.getByTestId("receipt")).toHaveCount(0, { timeout: 15000 });
  await expect(page.locator(".receipt")).toHaveCount(0, { timeout: 15000 });
  await shot(page, "6-archived");
});

test("an empty journey", async ({ page }) => {
  await page.clock.setFixedTime(new Date("2026-10-01T12:00:00Z"));
  await startOnSummary(page, undefined, "A journey to plan");
  await expect(page.getByTestId("summary-empty")).toBeVisible();
  await beat(page, 500);
  await shot(page, "7-empty");
});

// A phone is a touch device: nothing waits for a hover, so each row's actions are on it.
const PHONE = { viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true } as const;

test.describe("on a phone", () => {
  test.use(PHONE);

  test("a journey part way through", async ({ page }) => {
    await openAt(page, "browser", "j_vendor_eval", "summary", { fixedToday: VENDOR_DAY });
    await ready(page);
    await shot(page, "8-phone");
    await tall(page);
    await shot(page, "8-phone-full");
  });

  test("a long journey on a phone", async ({ page }) => {
    test.setTimeout(120000);
    await longJourney(page);
    await expect(page.getByTestId("receipt")).toHaveCount(0, { timeout: 15000 });
    await tall(page);
    await shot(page, "11-long-phone");
  });
});

const LONG_OWNER = "Dr. Alexandria Montgomery-Featherstonehaugh III, Director of Platform Engineering";
const LONG_TITLES = [
  "Reconcile the quarterly vendor invoice batches against the signed statements of work and the negotiated rate card, then file the exceptions",
  "Draft the migration runbook for the regional data stores, including the rollback criteria and the on-call hand-off between the two teams",
  "Collect the sign-off of every department head on the revised access-control policy before the review window closes",
  "Verify the backups restore",
  "Update the onboarding checklist",
  "Archive the old dashboards",
  "Rotate the shared credentials and record who holds each new one",
  "Review the incident timeline with the people who were on call",
  "Rewrite the status page copy",
  "Measure the cold-start time again after the configuration change",
];

/** A journey of long titles, long owner names and more than five rows a list, on its Summary, with three rows owned by the long name. */
async function longJourney(page: Page): Promise<void> {
  await open(page, "browser", "/entities");
  await addEntity(page, LONG_OWNER);
  const nodes: unknown[] = LONG_TITLES.map((title, at) => ({ key: `n_w${String(at)}`, id: `w${String(at)}`, kind: "action", title }));
  for (let at = 0; at < 7; at += 1) {
    nodes.push({ key: `n_q${String(at)}`, id: `q${String(at)}`, kind: "decision", title: `Which of the ${String(at + 2)} remaining options for the regional rollout should the programme commit to this quarter?`, prompt: "Which?", answer_type: "text" });
  }
  nodes.push({ key: "n_end", id: "end", kind: "milestone", title: "The programme closes with every regional store migrated and the old dashboards retired", final: true, requires: ["w0"] });
  await importRoute(page, "browser", { name: "long.yaml", mimeType: "application/yaml", buffer: Buffer.from(JSON.stringify({ format: 1, route: "long-list", name: "Long list", nodes })) });
  await page.getByRole("button", { name: "Publish the draft" }).click();
  const id = await startJourney(page, "browser", "A programme with long names", { route: "long-list", version: 1 });
  await goWithin(page, `/journeys/${id}`);
  await ready(page);
  // Assigned rows may leave Next up, so take the first row that still has a picker, three times.
  for (let turn = 0; turn < 3; turn += 1) {
    const row = page.locator('[data-testid="summary-next"] [data-testid="summary-item"]:has([data-testid="assign"])').first();
    await row.hover();
    await row.getByTestId("assign").selectOption({ label: LONG_OWNER });
    await beat(page, 400);
  }
}

test("a long journey: long titles, long owner names, more than five rows a list", async ({ page }) => {
  test.setTimeout(120000);
  await longJourney(page);
  await expect(page.getByTestId("receipt")).toHaveCount(0, { timeout: 15000 });
  await page.setViewportSize({ width: 1400, height: 1800 });
  await beat(page, 600);
  await shot(page, "11-long");
});

test("the main flow, on video", async ({ page }) => {
  await page.setViewportSize({ width: 1100, height: 680 });
  await openAt(page, "browser", "j_launch", "summary", { fixedToday: LAUNCH_DAY });
  await ready(page);
  await page.screencast.start({ path: join(out, "main-flow.webm") });
  await beat(page, 1500);
  // Done on an item of Waiting on you: the row leaves and the lead moves.
  const row = page.getByTestId("summary-yours").locator('[data-testid="summary-item"][data-node="n_announcement"]');
  await row.hover();
  await beat(page, 800);
  await row.getByRole("button", { name: "Done", exact: true }).click();
  await expect(row).toHaveCount(0);
  await beat(page, 1800);
  // Show all on a list that holds more than five.
  const more = page.getByTestId("rows-show-all").first();
  if (await more.isVisible()) {
    await more.click();
    await beat(page, 1500);
  }
  // A row's title opens the node on the graph, and Back returns to the Summary.
  const target = rowsOf(page, "summary-yours").first().locator("a.node-title");
  await target.click();
  await expect(page.getByTestId("node-detail")).toBeVisible();
  await beat(page, 2000);
  await page.goBack();
  await expect(page.getByTestId("summary-lead")).toBeVisible();
  await beat(page, 1500);
  await page.screencast.stop();
});
