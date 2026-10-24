// A19, C16, C17: hard deletion on the in-browser host. The journey index's rows are picked with
// their checkboxes, a Cmd-click or a Shift-click range; the inspector lists them and deletes them
// together once every one is archived, asked once with the count. A route nothing uses is deleted
// from its `⋯`; one a journey follows offers no delete. What the patches carry is vitest's
// (journeys/lifecycle.test.ts).
import { checkpoint } from "./invariants.ts";
import { expect, test } from "./test.ts";

import { journeyName, openRoute, setStatus, startJourney } from "./around.ts";
import { newRoute, routeName } from "./authoring.ts";
import { open } from "./shell.ts";

test("A19, C16: picked journeys are deleted together, once every one is archived", async ({ page }) => {
  const names = [journeyName("Gone A"), journeyName("Gone B"), journeyName("Kept")] as const;
  const ids: string[] = [];
  for (const name of names) {
    ids.push(await startJourney(page, "browser", name));
  }
  const [first, second, kept] = ids as [string, string, string];
  for (const id of [first, second]) {
    await open(page, "browser", `/journeys/${id}?view=graph`);
    await setStatus(page, "archived");
  }
  await open(page, "browser", "/journeys?status=any");
  const row = (id: string) => page.locator(`[data-testid="journey-row"][data-journey="${id}"]`);
  await row(first).getByRole("checkbox").check();
  await row(kept).getByRole("checkbox").check();
  // A Cmd-click on the row's name picks it rather than opening it.
  await row(second).getByRole("link", { name: names[1] }).click({ modifiers: ["ControlOrMeta"] });
  await expect(page).toHaveURL(/\/journeys\?status=any$/);
  const panel = page.getByTestId("selection-panel");
  await expect(panel).toHaveAttribute("data-count", "3");
  await expect(row(second)).toHaveAttribute("data-selected", "true");
  // One that is not archived keeps Delete off, and says which.
  const deletion = panel.getByTestId("delete-journeys");
  await expect(deletion).toHaveAttribute("data-blocked", "true");
  await expect(deletion.getByTestId("delete-why")).toContainText(names[2]);
  await expect(deletion.getByTestId("delete-ask")).toBeDisabled();
  await panel.getByRole("button", { name: `Remove ${names[2]} from the selection` }).click();
  await expect(panel).toHaveAttribute("data-count", "2");
  await deletion.getByTestId("delete-ask").click();
  const confirm = panel.getByTestId("delete-confirm");
  await expect(confirm).toHaveAttribute("data-count", "2");
  await checkpoint(page);
  await confirm.getByTestId("delete-confirmed").click();
  await expect(row(first)).toHaveCount(0);
  await expect(row(second)).toHaveCount(0);
  await expect(row(kept)).toBeVisible();
  await expect(panel).toHaveCount(0);
});

test("A19, C17: a route nothing uses is deleted from its menu, and one a journey follows offers no delete", async ({ page }) => {
  await openRoute(page, "browser", "vendor-evaluation");
  await page.getByTestId("route-menu").click();
  await expect(page.getByTestId("menu-retire")).toBeVisible();
  await expect(page.getByTestId("menu-delete-route")).toHaveCount(0);
  await page.keyboard.press("Escape");
  const id = await newRoute(page, "browser", routeName("Unused"));
  await page.getByTestId("route-menu").click();
  await page.getByTestId("menu-delete-route").click();
  await page.getByTestId("delete-route-confirmed").click();
  await expect(page).toHaveURL(/\/routes$/);
  await expect(page.locator(`a[href="/routes/${id}"]`)).toHaveCount(0);
});

test("C16: a Shift-click picks every journey from the last one picked to the one clicked", async ({ page }) => {
  await open(page, "browser", "/journeys?status=any");
  const rows = page.getByTestId("journey-row");
  await expect(rows.nth(2)).toBeVisible();
  await rows.nth(0).getByRole("checkbox").check();
  await rows.nth(2).getByRole("link").first().click({ modifiers: ["Shift"] });
  await expect(page).toHaveURL(/\/journeys\?status=any$/);
  for (const at of [0, 1, 2]) {
    await expect(rows.nth(at)).toHaveAttribute("data-selected", "true");
  }
  await expect(page.getByTestId("selection-panel")).toHaveAttribute("data-count", "3");
});
