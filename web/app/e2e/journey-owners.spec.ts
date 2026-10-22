// C16: a journey names who owns it. Owners are set in edit mode from the deployment's identities,
// kept across a reload, and shown on the journey card, the Summary, and the index row; the index
// filters by an owner, and "mine" keeps a journey its viewer owns with no work in it.
import type { Page } from "@playwright/test";

import { expect, test } from "./test.ts";

import { journeyName } from "./around.ts";
import { open, openJourney, visit, whenSettled } from "./shell.ts";

const JOURNEY = "j_vendor_eval";

/** Adds the owner the "Add an owner" select lists at `position`, and returns the name it showed. */
async function addOwner(page: Page, position: number): Promise<string> {
  const select = page.getByTestId("header-editor").getByLabel("Add an owner");
  const options = select.locator("option:not([value=''])");
  const name = (await whenSettled(page, () => options.nth(position).textContent())) ?? "";
  await select.selectOption({ index: position + 1 });
  return name;
}

test("C16: owners are added in edit mode, kept on reload, named on the card, the Summary and the index row, and narrowed to by the Owner filter", async ({ page }) => {
  await openJourney(page, "browser", JOURNEY);
  await expect(page.getByTestId("journey-owners")).toHaveAttribute("data-owners", "0");

  await page.getByTestId("journey-menu").click();
  await page.getByTestId("menu-edit").click();
  const bar = page.getByTestId("authoring-bar");
  await bar.getByRole("button", { name: "Edit the name, description and owners" }).click();
  const editor = page.getByTestId("header-editor");
  const first = await addOwner(page, 0);
  const second = await addOwner(page, 0);
  await expect(editor.getByTestId("chip-value")).toHaveText([first, second]);

  // The draft is kept across a reload until it is saved.
  await page.reload();
  await expect(page.getByTestId("header-editor").getByTestId("chip-value")).toHaveText([first, second]);
  await page.getByTestId("header-editor").getByRole("button", { name: "Save" }).click();
  await expect(page.getByTestId("header-editor")).toHaveCount(0);

  const card = page.getByTestId("journey-owners");
  await expect(card).toHaveAttribute("data-owners", "2");
  for (const name of [first, second]) {
    await expect(card).toContainText(name === "You" ? "you" : name);
  }

  // Removing one leaves the other.
  await bar.getByRole("button", { name: "Edit the name, description and owners" }).click();
  await page.getByTestId("header-editor").getByRole("button", { name: `Remove ${second}` }).click();
  await page.getByTestId("header-editor").getByRole("button", { name: "Save" }).click();
  await expect(card).toHaveAttribute("data-owners", "1");

  // The Summary names the owners in About.
  await open(page, "browser", `/journeys/${JOURNEY}?view=summary`);
  await expect(page.getByTestId("journey-owners")).toContainText(first === "You" ? "you" : first);

  // The index row carries them, and the Owner filter narrows to the journeys one owns.
  await open(page, "browser", "/journeys");
  const row = page.getByRole("row").filter({ has: page.locator(`a[href^="/journeys/${JOURNEY}"]`) }).first();
  await expect(row.getByTestId("row-owners")).toContainText(first === "You" ? "you" : first);
  await page.getByLabel("Owner").selectOption({ label: first });
  await expect(page.getByTestId("row-owners")).toHaveCount(1);
});

test("C16, E4: a journey started with its viewer as owner stays under Mine with no work of theirs in it, and one started without an owner does not", async ({ page }) => {
  const [owned, unowned] = [journeyName("Kept"), journeyName("Plain")];
  for (const [name, owner] of [[owned, true], [unowned, false]] as const) {
    await open(page, "browser", "/journeys/new");
    const form = page.getByTestId("new-journey-form");
    await form.getByLabel("Name").fill(name);
    await form.getByLabel("Start from").selectOption("");
    if (owner) {
      await form.getByLabel("Add an owner").selectOption({ label: "You" });
      await expect(form.getByTestId("chip-value")).toHaveText(["You"]);
    }
    await form.getByRole("button", { name: "Start the journey" }).click();
    await expect(page).toHaveURL(/\/journeys\/j_[a-z0-9_-]+\?/);
  }
  await open(page, "browser", "/journeys?mine=1");
  const table = page.getByTestId("journey-table");
  await expect(table.getByRole("link", { name: owned })).toBeVisible();
  await expect(table.getByRole("link", { name: unowned })).toHaveCount(0);
  await expect(table.getByRole("row").filter({ hasText: owned }).getByTestId("row-owners")).toContainText("you");
});

test("C16: a new-journey draft kept before owners existed opens with no owners, its name intact", async ({ page }) => {
  await open(page, "browser", "/journeys/new");
  await page.evaluate(() => {
    sessionStorage.setItem("cairn:draft:new-journey:", JSON.stringify({ name: "Older draft", description: "", route: "", version: 0 }));
  });
  await visit(page, "browser", "/journeys/new");
  const form = page.getByTestId("new-journey-form");
  await expect(form.getByLabel("Name")).toHaveValue("Older draft");
  await expect(form.getByTestId("owners-field")).toBeVisible();
  await expect(form.getByTestId("chip-value")).toHaveCount(0);
});
