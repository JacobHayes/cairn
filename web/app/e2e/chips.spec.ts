// The multi-value email field of the identity form on the identities screen (H3): each email a removable chip, taken on
// Enter, comma, a paste or leaving the field; a bad or repeated one stays under its reason, selected so a paste replaces it;
// Backspace in the empty input removes the last; it saves the list as chips. On cairn demo. Splitting a pasted list
// is ui/chips.test.ts's.
import type { Page } from "@playwright/test";

import { open, openSheet } from "./shell.ts";
import { allowHoldingChecks, checksAsked, holdChecks, releaseChecks } from "./held-checks.ts";
import { checkpoint } from "./invariants.ts";
import { expect, test } from "./test.ts";

const field = (page: Page) => page.getByTestId("create-identity").getByLabel("Emails");
const chips = (page: Page) => page.getByTestId("create-identity").getByTestId("chip-value");

test("H3: emails are chips taken from the keyboard, refused with a reason, replaced by a paste, and saved as a list, on a desktop and a phone", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await open(page, "/identities/new");
  await expect(page.getByTestId("create-identity")).toBeAttached();
  await openSheet(page);
  await field(page).fill("Ann@Example.org");
  await field(page).press("Enter");
  // The input is read-only while the engine reads a value, so the next is typed once this one has joined.
  await expect(chips(page)).toHaveText(["ann@example.org"]);
  await field(page).pressSequentially("bob@example.org,");
  await field(page).fill("cy@example.org");
  await page.getByTestId("create-identity").getByLabel("Name").focus();
  await expect(chips(page)).toHaveText(["ann@example.org", "bob@example.org", "cy@example.org"]);
  // The chips wrap at a phone's width and nothing runs past the window.
  await page.setViewportSize({ width: 390, height: 844 });
  // Narrowing starts the sheet over at its peek, once the app has read the new width; opening it before then finds it not yet peeking.
  await expect(page.locator(".frame")).toHaveAttribute("data-sheet", "peek");
  await openSheet(page);
  await checkpoint(page);
  await page.setViewportSize({ width: 1280, height: 800 });
  // The sheet's pane is inert until the app has read the wider window (the stylesheet changes a frame sooner), and typing into an inert field goes nowhere.
  await expect(page.locator(".inspector-body[data-pane='inspector']")).not.toHaveAttribute("inert");
  await expect(chips(page)).toHaveCount(3);

  await field(page).fill("not-an-email");
  await field(page).press("Enter");
  await expect(page.getByTestId("chips-reason")).toContainText("not-an-email");
  await expect(field(page)).toHaveValue("not-an-email");
  await field(page).fill("BOB@example.org");
  await field(page).press("Enter");
  await expect(page.getByTestId("chips-reason")).toContainText("already");
  await expect(chips(page)).toHaveCount(3);
  // A refused value blocks the save rather than being dropped.
  await expect(page.getByRole("button", { name: "Add identity" })).toBeDisabled();
  // It is selected, so a paste replaces it, and Tab still leaves the field.
  await page.waitForFunction(() => {
    const active = document.activeElement as HTMLInputElement;
    return active.value === "BOB@example.org" && active.selectionStart === 0 && active.selectionEnd === 15;
  });
  await page.evaluate(() => {
    const data = new DataTransfer();
    data.setData("text", "fixed@example.org");
    document.activeElement?.dispatchEvent(new ClipboardEvent("paste", { clipboardData: data, bubbles: true, cancelable: true }));
  });
  await expect(chips(page)).toHaveText(["ann@example.org", "bob@example.org", "cy@example.org", "fixed@example.org"]);
  await field(page).fill("bad again");
  await field(page).press("Tab");
  await expect(field(page)).not.toBeFocused();
  await expect(page.getByTestId("chips-reason")).toBeVisible();

  await field(page).fill("");
  await field(page).press("Backspace");
  await field(page).press("Backspace");
  await expect(chips(page)).toHaveText(["ann@example.org", "bob@example.org"]);
  await page.getByRole("button", { name: "Remove ann@example.org" }).click();
  await expect(chips(page)).toHaveText(["bob@example.org"]);

  await page.getByTestId("create-identity").getByLabel("Name").fill("Chip person");
  await page.getByRole("button", { name: "Add identity" }).click();
  const row = page.getByTestId("identity").filter({ hasText: "Chip person" });
  await expect(row.getByTestId("identity-emails")).toHaveText("bob@example.org");
  const detail = page.getByTestId("identity-detail");
  await expect(detail.getByTestId("identity-emails")).toHaveText("bob@example.org");
  await detail.getByRole("button", { name: "Edit" }).click();
  await expect(detail.getByRole("button", { name: "Remove bob@example.org" })).toBeVisible();
});

test("H3: what is typed while the engine reads an email is kept, and its answer joins the form as it is then", async ({ page }) => {
  await allowHoldingChecks(page);
  await open(page, "/identities/new");
  await expect(page.getByTestId("create-identity")).toBeAttached();
  await openSheet(page);
  await holdChecks(page);
  await field(page).fill("ann@example.org");
  await field(page).press("Enter");
  // While it is read, the text stays in the box and holds the save, and a name typed meanwhile is not undone by the answer.
  await expect(field(page)).toHaveValue("ann@example.org");
  await expect(page.getByRole("button", { name: "Add identity" })).toBeDisabled();
  await page.getByTestId("create-identity").getByLabel("Name").fill("Ann");
  await releaseChecks(page);
  await expect(chips(page)).toHaveText(["ann@example.org"]);
  await expect(page.getByTestId("create-identity").getByLabel("Name")).toHaveValue("Ann");
});

test("H3: an email is read as it is typed: its reason shows once the typing rests and clears when it is fixed", async ({ page }) => {
  await open(page, "/identities/new");
  await expect(page.getByTestId("create-identity")).toBeAttached();
  await openSheet(page);
  // No Enter, separator or blur: the typing alone brings the reason.
  await field(page).pressSequentially("not-an-email");
  await expect(page.getByTestId("chips-reason")).toContainText("not-an-email");
  await field(page).pressSequentially("@example.org");
  await expect(page.getByTestId("chips-reason")).toHaveCount(0);
  await expect(chips(page)).toHaveCount(0);
});

test("H3: typing asks the engine once the typing rests, not at every key", async ({ page }) => {
  await allowHoldingChecks(page);
  await page.clock.install();
  await open(page, "/identities/new");
  await expect(page.getByTestId("create-identity")).toBeAttached();
  await openSheet(page);
  await page.clock.runFor(1000);
  const before = await checksAsked(page);
  await field(page).pressSequentially("typed-quickly@example.org", { delay: 10 });
  // Nothing has rested yet; once it has, the one question is about the whole text.
  expect(await checksAsked(page)).toBe(before);
  await page.clock.runFor(1000);
  await expect.poll(() => checksAsked(page)).toBe(before + 1);
  await page.clock.runFor(1000);
  expect(await checksAsked(page)).toBe(before + 1);
});
