// PRD: C20. The tour on the server host: the record is the user's, so a second tab sees the run
// the first one started and no offer after it ends; the assistant step is there because the demo
// offers an assistant, and it is an example: no request to the journey assistant is made for the
// practice journey across the whole tour. Tagged @serial: the record is one per user, and the
// demo's user is the same for every test.
import type { Page } from "@playwright/test";

import { expect, test } from "./test.ts";

import { setFrontier } from "./acting.ts";
import { nodePanel, open, openFromCanvas } from "./shell.ts";

const callout = (page: Page) => page.getByTestId("tour-callout");

/** Opens the tour from You, which offers it whatever the record says. */
async function begin(page: Page): Promise<void> {
  await open(page, "server", "/me");
  await page.getByTestId("you-tour").click();
  await page.getByTestId("tour-start").click();
  await expect(callout(page)).toHaveAttribute("data-step", "map");
}

test("the assistant step is an example, and a second tab shares the run", { tag: ["@server", "@serial"] }, async ({ page, context }) => {
  const assistant: string[] = [];
  page.on("request", (request) => {
    if (new URL(request.url()).pathname.endsWith("/assistant")) {
      assistant.push(request.url());
    }
  });
  await begin(page);
  await expect(page.getByTestId("tour-label")).toContainText("of 6");
  // A second tab for the same user finds the run waiting, not an offer to start another.
  const other = await context.newPage();
  await open(other, "server", "/");
  await expect(other.getByTestId("tour-continue")).toBeVisible();
  await expect(other.getByTestId("tour-welcome")).toHaveCount(0);
  await other.close();

  await openFromCanvas(page, "n_outside");
  await page.getByTestId("tour-next").click();
  const panel = nodePanel(page, "n_outside");
  await panel.getByRole("radio", { name: /^yes\b/i }).check();
  await panel.getByRole("button", { name: "Save", exact: true }).click();
  await expect(callout(page)).toHaveAttribute("data-phase", "reveal");
  await page.getByTestId("tour-next").click();
  await setFrontier(page, true);
  await page.getByTestId("tour-next").click();
  await page.locator('tr[data-node="n_draft"]').getByRole("button", { name: /done/i }).click();
  await expect(callout(page)).toHaveAttribute("data-phase", "reveal");
  await page.getByTestId("tour-next").click();
  await expect(callout(page)).toHaveAttribute("data-step", "dates");
  await page.getByTestId("tour-next").click();
  await expect(callout(page)).toHaveAttribute("data-step", "assistant");
  await expect(page.getByTestId("tour-next")).toHaveAttribute("aria-disabled", "true");
  await page.getByTestId("tab-assistant").click();
  const example = page.getByTestId("tour-assistant-example");
  await example.getByTestId("tour-assistant-show").click();
  const reply = example.getByTestId("assistant-reply");
  await expect(reply).toBeVisible();
  // The reply grows the example's pane, and the hole grows with it: the whole reply is inside it.
  await expect(callout(page)).toHaveAttribute("data-phase", "reveal");
  await expect.poll(async () => {
    const [hole, shown] = [await page.getByTestId("tour-ring").boundingBox(), await reply.boundingBox()];
    return hole !== null && shown !== null && shown.y >= hole.y - 1 && shown.y + shown.height <= hole.y + hole.height + 1;
  }).toBe(true);
  await expect(page.getByTestId("tour-next")).toHaveText("Finish the tour");
  await page.getByTestId("tour-next").click();
  await expect(page.getByTestId("tour-end")).toBeVisible();
  expect(assistant).toEqual([]);

  // Finished: no offer anywhere but the rail-less You page, which offers it again.
  const again = await context.newPage();
  await open(again, "server", "/");
  await expect(again.getByTestId("tour-welcome")).toHaveCount(0);
  await expect(again.getByTestId("tour-continue")).toHaveCount(0);
  await again.close();
});

test("two tabs pressing Start together share one practice journey", { tag: ["@server", "@serial"] }, async ({ page, context }) => {
  await open(page, "server", "/me");
  await page.getByTestId("you-tour").click();
  const other = await context.newPage();
  await open(other, "server", "/me");
  await other.getByTestId("you-tour").click();
  await Promise.all([page.getByTestId("tour-start").click(), other.getByTestId("tour-start").click()]);
  const journeyOf = (tab: Page) => /\/journeys\/(j_[^/?]+)/.exec(tab.url())?.[1];
  // The winner is on its journey once the journey exists; the loser may be anywhere.
  await expect.poll(() => journeyOf(page) ?? journeyOf(other)).toBeDefined();
  // Whichever tab lost the race finds the winner's run waiting; both resume the one journey.
  for (const tab of [page, other]) {
    await tab.goto("/");
    await expect(tab.getByTestId("tour-continue")).toBeVisible();
    await tab.getByTestId("tour-resume").click();
    await expect.poll(() => journeyOf(tab)).toBeDefined();
  }
  expect(journeyOf(other)).toBe(journeyOf(page));
  await page.getByTestId("tour-skip").click();
  await expect(page).toHaveURL(/\/$/);
  await other.close();
});
