// The shell on the in-browser host (ARCHITECTURE, Web UI: in-browser host): no server, the
// fixtures seeded on load, each journey derived in the derive worker, a patch applied by the
// page's own service, and a draft that survives a reload.
import { expect, test } from "@playwright/test";

import { derivedRevision, fresh, open, openJourney, rename, renameOf, startRename } from "./shell.ts";

test("the journey index lists every fixture's journey", async ({ page }) => {
  await open(page, "browser");
  await expect(page.getByTestId("host")).toHaveAttribute("data-status", "browser");
  await expect(page.getByTestId("journey-row")).toHaveCount(4);
  for (const name of ["Two-week bake-off", "Hire a platform engineer", "Launch the reporting release"]) {
    await expect(page.getByRole("link", { name })).toBeVisible();
  }
});

test("a journey's document is derived in the worker", async ({ page }) => {
  await open(page, "browser");
  await page.getByRole("link", { name: "Hire a platform engineer" }).click();
  await expect(page.getByTestId("journey-name")).toHaveText("Hire a platform engineer");
  expect(await derivedRevision(page)).toBe(6);
  await expect(page.getByTestId("node-card")).toHaveCount(14);
  await expect(page.locator("[data-testid=node-card][data-here]").first()).toBeVisible();
  expect(page.workers().length).toBeGreaterThan(0);
});

test("a patch applies through the page's service and the view follows", async ({ page }) => {
  await openJourney(page, "browser", "j_launch");
  const before = await derivedRevision(page);
  await rename(page, "n_docs", fresh("Docs"));
  await expect(page.getByTestId("notice")).toHaveAttribute("data-tone", "saved");
  await expect.poll(() => derivedRevision(page)).toBe(before + 1);
});

test("a draft survives a reload", async ({ page }) => {
  await openJourney(page, "browser", "j_bakeoff");
  const draft = fresh("Unsent");
  await startRename(page, "n_summary", draft);
  await page.reload();
  await expect(page.getByTestId("derivation")).toBeVisible();
  await expect(renameOf(page, "n_summary").getByRole("textbox")).toHaveValue(draft);
});

test("every fixture's journey is derived in the worker", async ({ page }) => {
  await open(page, "browser");
  const rows = page.getByTestId("journey-row");
  await expect(rows).toHaveCount(4);
  const listed = await rows.evaluateAll((found) =>
    found.map((row) => ({
      id: row.querySelector(".mono")?.textContent ?? "",
      revision: Number(row.querySelector("td:last-child")?.textContent),
    })),
  );
  expect(listed.map((journey) => journey.id)).toContain("j_vendor_eval");
  for (const journey of listed) {
    await openJourney(page, "browser", journey.id);
    expect(await derivedRevision(page)).toBe(journey.revision);
  }
});
