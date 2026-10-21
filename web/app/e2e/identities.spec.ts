// E6, B3, H3: the identities page. Add opens a form in the inspector; a row opens an identity
// alone and the checkboxes make a set, merged or deleted from the inspector; Delete is off while
// anything names an identity. The open proposals are the inspector's Proposals tab (server.spec).
import { addIdentity, identityRow, mergeIdentities, selectIdentities } from "./around.ts";
import { fresh, open, openSheet } from "./shell.ts";
import { checkpoint, expect, test } from "./test.ts";

/** Ana Ruiz and Dara Singh of the fixtures' people: named by journeys. */
const ANA = "who_k3d7qm2xw5tn6pfa";
const DARA = "who_d4m6uk2eqf5rbhzs";

test("E6: a row opens that identity alone, the checkboxes make a set the inspector merges, and the search keeps the identities whose name or email holds it", async ({ page }) => {
  await open(page, "browser", "/identities");
  // The search keeps the identities whose name or email holds it, and says how many.
  await expect(page.getByTestId("identity").first()).toBeVisible();
  const all = await page.getByTestId("identity").count();
  expect(all).toBeGreaterThan(2);
  await page.getByLabel("Search").fill("ben@");
  await expect(page.getByTestId("identity")).toHaveCount(1);
  await expect(page.getByTestId("identities-count")).toContainText(String(all));
  await page.getByLabel("Search").fill("no such person");
  await expect(page.getByTestId("identities-none-match")).toBeVisible();
  await page.getByLabel("Search").fill("");
  await expect(page.getByTestId("identity")).toHaveCount(all);
  const [first, second, third] = [await addIdentity(page, fresh("Alpha")), await addIdentity(page, fresh("Beta")), await addIdentity(page, fresh("Gamma"))];
  await identityRow(page, first).getByTestId("identity-name").click();
  await expect(page).toHaveURL(new RegExp(`/identities/${first}$`));
  await expect(page.getByTestId("identity-detail")).toBeVisible();
  await selectIdentities(page, first, second);
  await expect(page.getByTestId("selection-panel")).toHaveAttribute("data-count", "2");
  await identityRow(page, third).getByRole("checkbox").check();
  await expect(page.getByTestId("selection-panel")).toHaveAttribute("data-count", "3");
  await page.getByRole("button", { name: /^Remove .* from the selection$/ }).first().click();
  await expect(page.getByTestId("selection-panel")).toHaveAttribute("data-count", "2");
  await page.getByTestId("selection-clear").click();
  await expect(page.getByTestId("selection-panel")).toHaveCount(0);
  await expect(page).toHaveURL(/\/identities$/);
  // A plain click on a member's name, with the address already on it, narrows the set to that one.
  await selectIdentities(page, first, second);
  await expect(page.getByTestId("selection-panel")).toBeVisible();
  await identityRow(page, first).getByTestId("identity-name").click();
  await expect(page.getByTestId("selection-panel")).toHaveCount(0);
  await expect(page.getByTestId("identity-detail")).toBeVisible();
  await expect(identityRow(page, second)).toHaveAttribute("data-selected", "false");
  await checkpoint(page);
  // Merging two selected identities leaves one row, which now carries the journeys of the other.
  await open(page, "browser", `/identities/${DARA}`);
  await expect(page.getByTestId("identity-detail")).toBeAttached();
  await openSheet(page);
  const references = page.locator("[data-rows-panel]").first();
  await expect(references).toBeVisible();
  const before = (await references.getAttribute("data-count")) ?? "";
  expect(Number(before)).toBeGreaterThan(0);
  const keeper = await addIdentity(page, fresh("Keeper"));
  await expect(page.getByTestId("delete-identities")).toHaveAttribute("data-deletion", "allowed");
  await mergeIdentities(page, keeper, DARA);
  await expect(identityRow(page, DARA)).toHaveCount(0);
  await expect(identityRow(page, keeper)).toHaveAttribute("data-selected", "true");
  await expect(page.locator("[data-rows-panel]").first()).toHaveAttribute("data-count", before);
});

test("E6: an identity nothing names is deleted from its inspector, one a journey names cannot be, and Delete over several waits for every one's read", async ({ page }) => {
  await open(page, "browser", "/identities");
  const spare = await addIdentity(page, fresh("Spare"));
  await expect(page.getByTestId("delete-identities")).toHaveAttribute("data-deletion", "allowed");
  await page.getByTestId("delete-identities").getByRole("button", { name: "Delete", exact: true }).click();
  await expect(identityRow(page, spare)).toHaveCount(0);
  await expect(page).toHaveURL(/\/identities$/);

  await identityRow(page, ANA).getByTestId("identity-name").click();
  await expect(page.getByTestId("identity-detail")).toBeAttached();
  await openSheet(page);
  const references = page.getByTestId("identity-references");
  await expect(references).toBeVisible();
  await expect(page.getByTestId("delete-identities")).toHaveAttribute("data-deletion", "referenced");
  await expect(page.getByTestId("delete-identities").getByRole("button", { name: "Delete", exact: true })).toBeDisabled();
  // Each journey or route that names her is a link to it.
  const links = references.getByRole("link");
  await expect(links.first()).toHaveAttribute("href", /^\/(journeys|routes)\//);
  await checkpoint(page);
  // Delete over several waits for every one's read, and stays off while any is named.
  const another = await addIdentity(page, fresh("Spare"));
  await selectIdentities(page, another, ANA);
  await expect(page.getByTestId("delete-identities")).toHaveAttribute("data-deletion", "referenced");
  await expect(page.getByTestId("delete-identities").getByRole("button", { name: /^Delete/ })).toBeDisabled();
  await page.getByRole("button", { name: /^Remove Ana Ruiz from the selection$/ }).click();
  await expect(page.getByTestId("delete-identities")).toHaveAttribute("data-deletion", "allowed");
});

test("E6: on a phone a row opens its identity, and the form's fields and Save stay reachable with the keyboard up", async ({ page }) => {
  // A window as short as a phone with its keyboard open.
  await page.setViewportSize({ width: 390, height: 460 });
  await open(page, "browser", "/identities");
  const key = await addIdentity(page, fresh("Phone person"));
  await expect(page.getByTestId("identity-detail")).toBeVisible();
  await openSheet(page);
  await page.getByTestId("identity-detail").getByRole("button", { name: "Edit" }).click();
  const name = fresh("Renamed");
  await page.getByTestId("identity-detail").getByLabel("Name").fill(name);
  const save = page.getByTestId("identity-detail").getByRole("button", { name: "Save" });
  // The inspector scrolls inside the sheet, so Save is one scroll away, never past the window or under the sheet.
  await save.scrollIntoViewIfNeeded();
  await expect(save).toBeInViewport({ ratio: 0.95 });
  await save.click();
  await expect(identityRow(page, key).getByTestId("identity-name")).toHaveText(name);
});
