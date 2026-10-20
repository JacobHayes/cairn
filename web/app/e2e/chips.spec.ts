// The multi-value email field on the identities screen (H3): each email a removable chip, taken on
// Enter, comma, a paste or leaving the field; a bad or repeated one stays under its reason; Backspace
// in the empty input removes the last; it saves the list as chips. On the in-browser host, desktop and phone.
import type { Page } from "@playwright/test";

import { open } from "./shell.ts";
import { expect, test } from "./test.ts";

const field = (page: Page) => page.getByTestId("create-identity").getByLabel("Emails");
const chips = (page: Page) => page.getByTestId("create-identity").getByTestId("chip-value");

for (const [name, viewport] of [["desktop", { width: 1280, height: 800 }], ["phone", { width: 390, height: 844 }]] as const) {
  test(`H3: emails are chips taken from the keyboard, refused with a reason, and saved as a list (${name})`, async ({ page }) => {
    await page.setViewportSize(viewport);
    await open(page, "browser", "/identities");
    await field(page).fill("Ann@Example.org");
    await field(page).press("Enter");
    await field(page).pressSequentially("bob@example.org,");
    await field(page).fill("cy@example.org");
    await page.getByTestId("create-identity").getByLabel("Name").focus();
    await expect(chips(page)).toHaveText(["ann@example.org", "bob@example.org", "cy@example.org"]);

    await field(page).fill("not-an-email");
    await field(page).press("Enter");
    await expect(page.getByTestId("chips-reason")).toContainText("not-an-email");
    await expect(field(page)).toHaveValue("not-an-email");
    await field(page).fill("BOB@example.org");
    await field(page).press("Enter");
    await expect(page.getByTestId("chips-reason")).toContainText("already");
    await expect(chips(page)).toHaveCount(3);
    // A refused value blocks the save rather than being dropped.
    await expect(page.getByRole("button", { name: "Add an identity" })).toBeDisabled();

    await field(page).fill("");
    await field(page).press("Backspace");
    await expect(chips(page)).toHaveText(["ann@example.org", "bob@example.org"]);
    await page.getByRole("button", { name: "Remove ann@example.org" }).click();
    await expect(chips(page)).toHaveText(["bob@example.org"]);

    await page.getByTestId("create-identity").getByLabel("Name").fill("Chip person");
    await page.getByRole("button", { name: "Add an identity" }).click();
    const row = page.getByTestId("identity").filter({ hasText: "Chip person" });
    await expect(row.getByTestId("identity-emails")).toHaveText("bob@example.org");
    await row.getByRole("button", { name: "Edit" }).click();
    await expect(row.getByRole("button", { name: "Remove bob@example.org" })).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  });
}

test("H3: a pasted list becomes chips, one per address", async ({ page }) => {
  await open(page, "browser", "/identities");
  await field(page).focus();
  await page.evaluate(() => {
    const data = new DataTransfer();
    data.setData("text", "a@example.org\nb@example.org, c@example.org");
    document.activeElement?.dispatchEvent(new ClipboardEvent("paste", { clipboardData: data, bubbles: true, cancelable: true }));
  });
  await expect(chips(page)).toHaveText(["a@example.org", "b@example.org", "c@example.org"]);
});

test("H3: a refused value is selected so a paste replaces it, and Tab still leaves the field", async ({ page }) => {
  await open(page, "browser", "/identities");
  await field(page).fill("bad");
  await field(page).press("Enter");
  await expect(page.getByTestId("chips-reason")).toContainText("bad");
  await page.waitForFunction(() => {
    const active = document.activeElement as HTMLInputElement;
    return active.value === "bad" && active.selectionStart === 0 && active.selectionEnd === 3;
  });
  await page.evaluate(() => {
    const data = new DataTransfer();
    data.setData("text", "fixed@example.org");
    document.activeElement?.dispatchEvent(new ClipboardEvent("paste", { clipboardData: data, bubbles: true, cancelable: true }));
  });
  await expect(chips(page)).toHaveText(["fixed@example.org"]);
  await field(page).fill("bad again");
  await field(page).press("Tab");
  await expect(field(page)).not.toBeFocused();
  await expect(page.getByTestId("chips-reason")).toBeVisible();
});

test("the merge is folded until asked for", async ({ page }) => {
  await open(page, "browser", "/identities");
  await expect(page.getByTestId("merge-identities")).toHaveCount(0);
  await page.getByRole("button", { name: "Merge identities" }).click();
  await expect(page.getByTestId("merge-identities")).toBeVisible();
  await page.getByRole("button", { name: "Close merge" }).click();
  await expect(page.getByTestId("merge-identities")).toHaveCount(0);
});
