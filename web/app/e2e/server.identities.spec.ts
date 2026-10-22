// E6, H6: the Identities page when the server answers late. A references read that is still out
// when the selection moves must never speak for the new selection, and a merge whose reply
// comes after the deployment has already moved still ends on the server's state. Each test holds
// a response with the request routing and releases it when the page has moved on, so it needs the
// server host.
import type { Page, Route } from "@playwright/test";

import { expect, test } from "./test.ts";

import { addIdentity, identityRow } from "./around.ts";
import { fresh, open, openSheet, whenSettled } from "./shell.ts";

/** Holds every request for `path` until `release()`: the page has asked, and hears nothing. */
async function hold(page: Page, path: string): Promise<{ release: () => Promise<void>; held: () => number }> {
  const waiting: Route[] = [];
  let open = false;
  await page.route((url) => url.pathname === path, (route) => {
    if (open) {
      void route.continue();
    } else {
      waiting.push(route);
    }
  });
  return {
    // The held requests go to the server; this returns once the page has the answers whole. What the page made of them is
    // read from what it showed (`deletionStates`), not from a moment after.
    release: async () => {
      const answered = Promise.all(waiting.map(() => page.waitForResponse((response) => new URL(response.url()).pathname === path).then((response) => response.finished())));
      open = true;
      for (const route of waiting.splice(0)) {
        void route.continue();
      }
      await answered;
    },
    held: () => waiting.length,
  };
}

/**
 * Records every state the Delete control shows from now on, in the page, and returns a read of
 * them in order: a late answer that spoke for the selection even for one render is in it.
 */
async function deletionStates(page: Page): Promise<() => Promise<string[]>> {
  await page.evaluate(() => {
    const shown: string[] = [];
    const note = (state: string | null | undefined) => {
      if (state !== null && state !== undefined && shown.at(-1) !== state) {
        shown.push(state);
      }
    };
    const current = () => document.querySelector('[data-testid="delete-identities"]')?.getAttribute("data-deletion");
    note(current());
    new MutationObserver((records) => {
      for (const record of records) {
        if (record.attributeName === "data-deletion") {
          note(record.oldValue);
        }
      }
      note(current());
    }).observe(document.body, { subtree: true, childList: true, attributes: true, attributeFilter: ["data-deletion"], attributeOldValue: true });
    (window as unknown as { deletionShown: string[] }).deletionShown = shown;
  });
  return () => whenSettled(page, () => page.evaluate(() => (window as unknown as { deletionShown: string[] }).deletionShown));
}

const referencesOf = (key: string) => `/api/deployment/identities/${key}/references`;
const ANA = "who_k3d7qm2xw5tn6pfa";

test("E6: a references read still out for one identity never speaks for the next, and one out while the selection grows keeps Delete off until the whole set has answered", { tag: ["@server", "@serial"] }, async ({ page }) => {
  await open(page, "server", "/identities");
  const spare = await addIdentity(page, fresh("Spare"));
  await expect(page.getByTestId("delete-identities")).toHaveAttribute("data-deletion", "allowed");
  // A, nothing names it, is read slowly; B, which journeys name, is read slowly too.
  await page.goto("/identities");
  const slowSpare = await hold(page, referencesOf(spare));
  const slowAna = await hold(page, referencesOf(ANA));
  await identityRow(page, spare).getByTestId("identity-name").click();
  await expect.poll(slowSpare.held).toBeGreaterThan(0);
  await identityRow(page, ANA).getByTestId("identity-name").click();
  await expect.poll(slowAna.held).toBeGreaterThan(0);
  const deletion = page.getByTestId("delete-identities");
  await expect(deletion).toHaveAttribute("data-deletion", "checking");
  const shown = await deletionStates(page);
  // The answer for A (nothing names it) arrives while B is open and unanswered.
  await slowSpare.release();
  await expect(deletion).toHaveAttribute("data-deletion", "checking");
  await expect(deletion.getByRole("button", { name: "Delete", exact: true })).toBeDisabled();
  await expect(page.getByTestId("identity-references")).toHaveCount(0);
  // B's own answer is what it shows.
  await slowAna.release();
  await expect(deletion).toHaveAttribute("data-deletion", "referenced");
  await expect(page.getByTestId("identity-references")).toBeVisible();
  await expect(deletion.getByRole("button", { name: "Delete", exact: true })).toBeDisabled();
  // A's answer never showed, not even for a moment before B's.
  expect(await shown()).toEqual(["checking", "referenced"]);
  // The selection grows while the first read is out: its late answer says nothing about the pair.
  await page.goto("/identities");
  const lateSpare = await hold(page, referencesOf(spare));
  const lateAna = await hold(page, referencesOf(ANA));
  await identityRow(page, spare).getByTestId("identity-name").click();
  await expect.poll(lateSpare.held).toBeGreaterThan(0);
  await identityRow(page, ANA).getByRole("checkbox").check();
  await expect(page.getByTestId("selection-panel")).toBeVisible();
  await openSheet(page);
  await expect.poll(lateAna.held).toBeGreaterThan(0);
  await expect(deletion).toHaveAttribute("data-deletion", "checking");
  const shownForPair = await deletionStates(page);
  // The first read, of the spare one alone, answers late: it says nothing about the pair.
  await lateSpare.release();
  await expect(deletion).toHaveAttribute("data-deletion", "checking");
  await expect(deletion.getByRole("button", { name: /^Delete 2$/ })).toBeDisabled();
  await lateAna.release();
  await expect(deletion).toHaveAttribute("data-deletion", "referenced");
  await expect(deletion.getByRole("button", { name: /^Delete 2$/ })).toBeDisabled();
  expect(await shownForPair()).toEqual(["checking", "referenced"]);
});

test("E6: a merge whose reply comes after the deployment has moved settles on the server's state: the survivor open alone, the count down by one", { tag: ["@server", "@serial"] }, async ({ page }) => {
  await open(page, "server", "/identities");
  const first = await addIdentity(page, fresh("Early"));
  const second = await addIdentity(page, fresh("Late"));
  const shown = page.getByTestId("identities-count");
  const countOf = (text: string | null) => Number(/^\d+/.exec(text ?? "")?.[0]);
  const before = countOf(await whenSettled(page, () => shown.textContent()));
  await identityRow(page, first).getByRole("checkbox").check();
  await identityRow(page, second).getByRole("checkbox").check();
  await expect(page.getByTestId("selection-panel")).toBeVisible();
  await openSheet(page);
  const panel = page.getByTestId("merge-identities");
  await panel.locator(`input[data-identity="${second}"]`).check();
  // The server applies the merge and the page hears of the new deployment before it hears the merge's own answer.
  let reply: (() => void) | undefined;
  const replied = new Promise<void>((resolve) => {
    reply = resolve;
  });
  await page.route("**/api/deployment/patches", async (route) => {
    const response = await route.fetch();
    await replied;
    await route.fulfill({ response });
  });
  await panel.getByRole("button", { name: "Merge", exact: true }).click();
  await expect(identityRow(page, first)).toHaveCount(0);
  await expect.poll(async () => countOf(await shown.textContent())).toBe(before - 1);
  reply?.();
  await expect(page).toHaveURL(new RegExp(`/identities/${second}$`));
  await expect(page.getByTestId("selection-panel")).toHaveCount(0);
  await expect(page.getByTestId("identity-gone")).toHaveCount(0);
  await expect(page.getByTestId("identity-detail").getByTestId("aliases")).toBeVisible();
  await expect(identityRow(page, second)).toHaveAttribute("data-selected", "true");
  expect(countOf(await whenSettled(page, () => shown.textContent()))).toBe(before - 1);
  // The set is the survivor alone, not it and the identity merged away: unchecking it leaves nothing selected.
  await identityRow(page, second).getByRole("checkbox").uncheck();
  await expect(page).toHaveURL(/\/identities$/);
  await expect(page.getByTestId("identity-detail")).toHaveCount(0);
  await expect(page.getByTestId("identity-gone")).toHaveCount(0);
});
