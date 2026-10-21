// Brief 5.7 in the browser, on the in-browser host: the one proposal-review flow, the scenario
// journey upgraded to the fixture's version 2 with each kind of conflict that version raises
// resolved and applied (B7, C14), where the review screen meets the canvas, the inspector and
// the host. What a proposal offers, blocks and reads as stale is model and host tests'.
import type { Page } from "@playwright/test";

import { expect, test } from "./test.ts";

import { openJourneyCard } from "./around.ts";
import { apply, blockers, editWhereVersionTwoChanges, graphRoom, marginIn, pickNode, publishVersionTwo, resolve, reviewItem, reviewOpen, saveEdits, scrollsOnce } from "./proposals.ts";
import { expectAddress, goTo, nodeCard, open, setDepth, syncChip } from "./shell.ts";

/** Drafts the upgrade of the scenario journey to version 2 (after the edits version 2 also touches) and returns the review's address. */
async function draftUpgrade(page: Page): Promise<string> {
  await editWhereVersionTwoChanges(page, "browser");
  await publishVersionTwo(page, "browser");
  await openJourneyCard(page, "browser", "j_vendor_eval");
  await page.getByTestId("journey-upgrade").click();
  const flow = page.getByTestId("upgrade-flow");
  await expect(flow.getByLabel("Upgrade to version")).toHaveValue("2");
  await flow.getByRole("button", { name: "Propose the upgrade" }).click();
  await reviewOpen(page);
  const reviewed = new URL(page.url()).pathname;
  expect(reviewed).toMatch(/^\/journeys\/j_vendor_eval\/proposals\/pr_[0-9a-f]+$/);
  return reviewed;
}

/** The drafted upgrade is listed in its journey's Proposals tab and on Home, and opens under the journey. */
async function proposalIsListed(page: Page, reviewed: string): Promise<void> {
  // Away from it: the journey's tab names it, and its row opens the same address.
  await open(page, "browser", "/journeys/j_vendor_eval");
  await expect(page.getByTestId("tab-proposals")).toContainText("1");
  await page.getByTestId("tab-proposals").click();
  await page.getByTestId("proposals-pane").getByTestId("proposal-row").getByRole("link").click();
  await reviewOpen(page);
  await expectAddress(page, reviewed);
  // The address of the list opens the journey with the tab in view.
  await open(page, "browser", "/journeys/j_vendor_eval/proposals");
  await expect(page.getByTestId("tab-proposals")).toHaveAttribute("aria-selected", "true");
  await expect(page.getByTestId("proposals-panel").getByTestId("proposal-row")).toHaveCount(1);
  // Home waits on it too, in a fold after the journeys.
  await open(page, "browser", "/");
  await page.getByTestId("home-proposals").locator("summary").click();
  await page.getByTestId("home-proposals").getByTestId("proposal-row").getByRole("link").click();
  await reviewOpen(page);
  await expectAddress(page, reviewed);
  // The Proposals tab stays the pane's choice; the review's rail is under Inspector.
  await page.getByRole("tab", { name: "Inspector" }).click();
}

/** A change the first fit leaves out is brought into view from the rail, at the zoom it was at. */
async function changeBroughtIntoView(page: Page): Promise<void> {
  const zoom = (await graphRoom(page)).zoom;
  await page.getByTestId("changes-fold").locator("summary").click();
  await page.locator('[data-testid="diff-node"][data-node="n_signoff"] button').click();
  await expect(page.getByTestId("item-editor")).toBeVisible();
  await expect.poll(async () => (await marginIn(page, "n_signoff", await graphRoom(page))) >= 0).toBe(true);
  expect((await graphRoom(page)).zoom).toBeCloseTo(zoom, 2);
}

/** The drafted proposal is one click away in Recent, and a conflict still to decide holds Apply, for that reason alone. */
async function conflictHoldsApply(page: Page): Promise<void> {
  // The drafted proposal is also one click away in Recent, for when the screen that opened it is gone.
  await syncChip(page).click();
  await expect(page.getByTestId("sync-popover").getByTestId("sync-recent").filter({ hasText: "Drafted a proposal" }).getByRole("button", { name: "Review it" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("sync-popover")).toHaveCount(0);
  // Still a conflict to decide: Apply is held, for that reason alone, and pressing it shows why.
  await expect.poll(() => blockers(page)).toEqual(["unresolved"]);
  await expect(page.getByTestId("apply-proposal")).toHaveAttribute("aria-disabled", "true");
  await expect(page.getByTestId("to-decide")).toBeVisible();
  await expect(nodeCard(page, "n_signoff")).toHaveAttribute("data-diff", "added");
  await expect(nodeCard(page, "n_signoff").getByTestId("card-diff")).toContainText("Added");
  await page.getByTestId("apply-proposal").click({ force: true });
  await expect(page.getByTestId("apply-waits")).toBeFocused();
}

/** Each kind of conflict is read on its node. */
async function conflictsAreRead(page: Page): Promise<void> {
  await pickNode(page, "n_access");
  await expect(reviewItem(page, "conflict", "n_access")).toHaveAttribute("data-about", "field");
  await scrollsOnce(page);
  await pickNode(page, "n_baseline");
  await expect(reviewItem(page, "conflict", "n_baseline")).toHaveAttribute("data-about", "field");
  await pickNode(page, "n_kickoff");
  await expect(reviewItem(page, "kept_local_edit", "n_kickoff")).toBeVisible();
  await pickNode(page, "n_workload");
  await expect(reviewItem(page, "orphan", "n_workload")).toBeVisible();
}

/** Each conflict resolved, the removal confirmed, saved and applied. */
async function resolvedAndApplied(page: Page): Promise<void> {
  await resolve(page, "n_access", "keep_journey");
  await resolve(page, "n_baseline", "take_route");
  await pickNode(page, "n_workload");
  const orphan = reviewItem(page, "orphan", "n_workload");
  await orphan.getByTestId("orphan-remove").check();
  await expect(orphan.locator('[data-testid="removal-descendant"]')).toHaveCount(2);
  await goTo(page, "list");
  await expect(page.locator('[data-testid="list-row"][data-node="n_workload"]')).toHaveAttribute("data-diff", "removed");
  await expect(page.locator('[data-testid="list-row"][data-diff="removed"]')).toHaveCount(3);
  await saveEdits(page);
  await apply(page);
  await goTo(page, "graph");
  await setDepth(page, "Actions");
  await expect(nodeCard(page, "n_signoff")).toBeVisible();
  await expect(nodeCard(page, "n_access").getByTestId("title")).toHaveText("Access to the environment");
  await expect(nodeCard(page, "n_workload")).toHaveCount(0);
  // Close leaves the layer for the journey itself, the view kept.
  await page.getByTestId("close-layer").click();
  await expect(page.getByTestId("layer-band")).toHaveCount(0);
  expect(new URL(page.url()).pathname).toBe("/journeys/j_vendor_eval");
}

test("B7, C14, C16: the scenario journey upgraded to version 2 is listed on its journey and on Home, a change out of the first fit is brought into view, each conflict is resolved, then applied", async ({ page }) => {
  const reviewed = await draftUpgrade(page);
  await proposalIsListed(page, reviewed);
  await page.setViewportSize({ width: 1440, height: 768 });
  // The layer opens at the page's own depth; the changes the proposal makes inside the stages are reached at Actions.
  await setDepth(page, "Actions");
  await conflictHoldsApply(page);
  await changeBroughtIntoView(page);
  await conflictsAreRead(page);
  await resolvedAndApplied(page);
});

test("C14, A11: saving a journey as a route is reviewed over the route's own page, in every view, and applied", async ({ page }) => {
  await openJourneyCard(page, "browser", "j_vendor_eval");
  await page.getByTestId("journey-menu").click();
  await page.getByTestId("menu-save").click();
  await page.getByTestId("save-as-route-flow").getByLabel("Route id").fill("r_saved");
  await page.getByTestId("save-as-route-flow").getByRole("button", { name: "Propose saving it as a route" }).click();
  await reviewOpen(page);
  const layered = new URL(page.url()).pathname;
  expect(layered).toMatch(/^\/routes\/r_saved\/proposals\/pr_[0-9a-f]+$/);
  // The route's own views stay under the layer: a switch keeps the proposal in the address.
  // Every node of the route it creates is marked added, in the list and on the timeline alike.
  const added = { list: '[data-testid="route-row"][data-diff="added"]', timeline: '[data-diff="added"] [data-testid="timeline-words"]' };
  for (const to of ["list", "timeline"] as const) {
    await page.getByTestId("view-picker").click();
    await page.getByTestId(`view-${to}`).click();
    await expect(page.getByTestId("route-frame")).toHaveAttribute("data-screen", to);
    await expect(page.locator(added[to]).first()).toBeVisible();
    await expect(page.getByTestId("layer-band")).toHaveAttribute("data-mode", "reviewing");
    expect(new URL(page.url()).pathname).toBe(layered);
  }
  await apply(page);
  await page.getByTestId("close-layer").click();
  await expect(page.getByTestId("layer-band")).toHaveCount(0);
  expect(new URL(page.url()).pathname).toBe("/routes/r_saved");
});

// C14: the layer opens the graph framed on what the proposal changes, whole in the room the sheet leaves, and never so far out that a card's title cannot be read.
const LEGIBLE = 0.75;

/** Away and back at `size`, so the canvas opens afresh there: the first change is framed with a margin, legible and drawn to read. */
async function firstChangeFramed(page: Page, layered: string, size: { width: number; height: number }): Promise<void> {
  await page.setViewportSize(size);
  await open(page, "browser", "/journeys/j_vendor_eval");
  await open(page, "browser", `${layered}?depth=actions`);
  await reviewOpen(page);
  // The first change in the rail's order is the first conflict waiting on a choice.
  const first = await page.getByTestId("to-decide-item").first().evaluate((row) => row.querySelector("button")?.textContent ?? "");
  const card = page.locator('[data-testid="node-card"][data-diff]').filter({ has: page.getByTestId("title").filter({ hasText: first }) });
  await expect(card.first()).toBeVisible();
  const node = (await card.first().getAttribute("data-node")) ?? "";
  await expect.poll(async () => (await marginIn(page, node, await graphRoom(page))) >= 8).toBe(true);
  expect((await graphRoom(page)).zoom).toBeGreaterThanOrEqual(LEGIBLE - 0.001);
  await changeReads(page, node);
  if (size.width < 720) {
    // At the sheet's peek the rail already says what holds Apply.
    const [holds, sheet] = [await page.getByTestId("apply-waits").boundingBox(), await page.locator(".inspector").locator("xpath=..").boundingBox()];
    expect((holds?.y ?? 0) + (holds?.height ?? 0)).toBeLessThanOrEqual((sheet?.y ?? 0) + (sheet?.height ?? 0));
  } else {
    // The sync dot stays at the right edge of the brand row.
    const [chip, rail] = [await syncChip(page).boundingBox(), await page.locator(".rail-head").boundingBox()];
    expect((rail?.x ?? 0) + (rail?.width ?? 0) - ((chip?.x ?? 0) + (chip?.width ?? 0))).toBeLessThanOrEqual(24);
  }
}

/** What the layer's first view shows reads: the change's word whole on the card, the kind whole or cut with an ellipsis, and the title in full ink though the node is done. */
async function changeReads(page: Page, node: string): Promise<void> {
  const word = nodeCard(page, node).getByTestId("card-diff");
  expect(await word.evaluate((element) => element.scrollWidth <= element.clientWidth + 1 && element.parentElement !== null && element.getBoundingClientRect().right <= element.parentElement.getBoundingClientRect().right + 1)).toBe(true);
  // The kind either shows whole or keeps room for the ellipsis that says it is cut (never a bare letter).
  const kind = nodeCard(page, node).locator(".node-kind").first();
  expect(await kind.evaluate((element) => element.scrollWidth <= element.clientWidth + 1 || element.clientWidth >= 3 * parseFloat(getComputedStyle(element).fontSize) * 0.6)).toBe(true);
  expect(await kind.evaluate((element) => element.getBoundingClientRect().right <= (element.closest(".node-head-label")?.getBoundingClientRect().right ?? 0) + 1)).toBe(true);
  expect(await nodeCard(page, node).getByTestId("title").evaluate((element) => {
    const probe = Object.assign(document.createElement("span"), { textContent: "x" });
    probe.style.color = "var(--color-text)";
    element.append(probe);
    const ink = getComputedStyle(probe).color;
    probe.remove();
    return getComputedStyle(element).color === ink;
  })).toBe(true);
}

test("C14: on a desktop and then a phone the layer opens the graph on its first change, framed with a margin and legible", async ({ page }) => {
  await editWhereVersionTwoChanges(page, "browser");
  await publishVersionTwo(page, "browser");
  await openJourneyCard(page, "browser", "j_vendor_eval");
  await page.getByTestId("journey-upgrade").click();
  await page.getByTestId("upgrade-flow").getByRole("button", { name: "Propose the upgrade" }).click();
  await reviewOpen(page);
  const layered = new URL(page.url()).pathname;
  await firstChangeFramed(page, layered, { width: 1440, height: 768 });
  await firstChangeFramed(page, layered, { width: 390, height: 844 });
});
