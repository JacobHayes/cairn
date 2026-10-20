// The shell on the server host, against `cairn demo` (the `server` project of
// playwright.config.ts): what only a live server shows. Writes from different pages meet
// through the safe retry and the conflict (H5), a later page and a page on the index follow
// what another wrote (H6), a deployment tick re-derives what it moved, and an identity
// signed in at the demo's stub issuer links to the user (H3). Each test makes what it
// changes, so the fixtures other tests read stay as seeded.
import type { APIRequestContext } from "@playwright/test";

import { expect, test } from "./test.ts";

import { startVendorJourney } from "./acting.ts";
import { addEntity, mergeEntities } from "./around.ts";
import { addNode, newRoute, routeName } from "./authoring.ts";
import { section } from "./detail.ts";
import {
  countDocumentFetches,
  derivedRevision,
  fresh,
  goTo,
  live,
  nodeCard,
  open,
  openFromCanvas,
  openJourney,
  rename,
  renameOf,
  save,
  startRename,
  syncChip,
} from "./shell.ts";

const title = (page: Parameters<typeof nodeCard>[0], node: string) => nodeCard(page, node).getByTestId("title");

test("edits to different nodes both land, and edits to one field surface a conflict", { tag: "@server" }, async ({ context }) => {
  const [one, two] = [await context.newPage(), await context.newPage()];
  await openJourney(one, "server", "j_hiring");
  await openJourney(two, "server", "j_hiring");
  // A stale write to another node is retried and lands with no conflict.
  const [offer, screen] = [fresh("Offer"), fresh("Screen")];
  await startRename(two, "n_screen", screen);
  await rename(one, "n_offer", offer);
  await expect(title(two, "n_offer")).toHaveText(offer);
  await save(two, "n_screen");
  await expect(title(two, "n_screen")).toHaveText(screen);
  await expect(two.locator('[data-testid="refused"][data-rejection="stale"]')).toHaveCount(0);
  await expect(title(one, "n_screen")).toHaveText(screen);
  // One to the same field is not: it is shown beside the other's, and kept on the current version when asked.
  const [theirs, mine] = [fresh("Theirs"), fresh("Mine")];
  await startRename(two, "n_onsite", mine);
  await rename(one, "n_onsite", theirs);
  await save(two, "n_onsite");
  await expect(two.locator('[data-testid="refused"][data-rejection="stale"]')).toContainText(theirs);
  await expect(syncChip(two)).toHaveAttribute("aria-label", /^CONFLICT · 1\./);
  await expect(renameOf(two, "n_onsite").getByRole("textbox")).toHaveValue(mine);
  await expect(title(one, "n_onsite")).toHaveText(theirs);
  await two.getByRole("button", { name: "Keep my edit on the current version" }).click();
  await save(two, "n_onsite");
  await expect(title(two, "n_onsite")).toHaveText(mine);
  await expect(title(one, "n_onsite")).toHaveText(mine);
});

test("a later page shows the edit, and a page on the index shows a journey started elsewhere", { tag: "@server" }, async ({ context }) => {
  const [one, two] = [await context.newPage(), await context.newPage()];
  const fetched = countDocumentFetches(two, "j_launch");
  await openJourney(one, "server", "j_launch");
  await openJourney(two, "server", "j_launch");
  const renamed = fresh("Docs");
  await rename(one, "n_docs", renamed);
  await expect(title(two, "n_docs")).toHaveText(renamed);
  await two.getByRole("link", { name: "Journeys" }).click();
  await expect(two.getByRole("link", { name: "Launch the reporting release" })).toBeVisible();
  // The index is live: a journey started in the other page is listed with no reload.
  const started = await startVendorJourney(one);
  await expect(two.locator(`[data-testid="journey-row"][data-journey="${started}"]`)).toBeVisible();
  await two.getByRole("link", { name: "Launch the reporting release" }).click();
  await goTo(two, "graph");
  await expect(title(two, "n_docs")).toHaveText(renamed);
  await live(two);
  const later = await context.newPage();
  await openJourney(later, "server", "j_launch");
  await expect(title(later, "n_docs")).toHaveText(renamed);
  // H6: the page refetched once per newer revision, never for one it already held.
  expect(fetched()).toBe(2);
});

const patchId = () => `p_${crypto.randomUUID().replaceAll("-", "")}`;

/** An empty journey with one action owned by `owner`, made through the API; its id. */
async function journeyOwnedBy(request: APIRequestContext, owner: string): Promise<string> {
  const deployment = (await (await request.get("/api/deployment")).json()) as { revision: number };
  const id = `j_${fresh("owned").replace(/\W/g, "_").toLowerCase()}`;
  const response = await request.post(`/api/journeys/${id}/patches`, {
    data: {
      patch: {
        id: patchId(),
        target: { journey: id },
        base_revision: 0,
        deployment_revision: deployment.revision,
        mutations: [
          { op: "create_journey", name: "Owned work" },
          { op: "add_node", node: { key: "n_task", id: "task", kind: "action", title: "The task" } },
          { op: "set_participation", node: "n_task", kind: "k_owner", source: [owner] },
        ],
      },
    },
  });
  expect(response.ok(), await response.text()).toBe(true);
  return id;
}

test("a deployment tick refetches the deployment context and re-derives: a merge changes the owner another page shows", { tag: "@server" }, async ({ context }) => {
  const [one, two] = [await context.newPage(), await context.newPage()];
  const deploymentFetches: string[] = [];
  two.on("request", (request) => {
    if (new URL(request.url()).pathname === "/api/deployment") {
      deploymentFetches.push(request.url());
    }
  });
  await open(one, "server", "/entities");
  const first = await addEntity(one, fresh("Owner"));
  const second = await addEntity(one, fresh("Survivor"));
  const journey = await journeyOwnedBy(one.request, first);
  await openJourney(two, "server", journey);
  await live(two);
  const survivor = (await one.locator(`[data-testid="entity"][data-entity="${second}"]`).getByTestId("entity-name").textContent()) ?? "";
  // The card names an owner only when it is the viewer or missing: the node's detail names this one.
  const owners = await section(await openFromCanvas(two, "n_task"), "participations");
  await expect(owners).not.toContainText(survivor);
  const [before, fetchedBefore] = [await derivedRevision(two), deploymentFetches.length];
  await mergeEntities(one, second, first);
  const deployment = (await (await one.request.get("/api/deployment")).json()) as { revision: number };
  await expect(syncChip(two)).toHaveAttribute("data-deployment", String(deployment.revision));
  await expect(owners).toContainText(survivor);
  expect(await derivedRevision(two)).toBe(before);
  expect(deploymentFetches.length).toBe(fetchedBefore + 1);
});

test("an identity signed in with the stub issuer links to the user, and its verified email names their entity", { tag: "@server" }, async ({ page }) => {
  const email = `${fresh("linked").replace(/\W/g, "-")}@example.org`;
  await open(page, "server", "/entities");
  const entity = await addEntity(page, fresh("Linked person"), [email]);
  await open(page, "server", "/me");
  await expect(page.getByTestId("identity")).toHaveCount(1);
  await expect(page.locator(`[data-testid="your-entity"][data-entity="${entity}"]`)).toHaveCount(0);
  await page.getByTestId("link-identity").getByRole("link", { name: "Sign in with stub" }).click();
  await page.getByLabel("Email").fill(email);
  await page.getByRole("button", { name: "Sign in" }).click();
  await expect(page.getByTestId("identity-screen")).toBeVisible();
  await expect(page.locator('[data-testid="identity"][data-provider="stub"]').getByTestId("identity-emails")).toContainText(email);
  await expect(page.locator('[data-testid="identity"][data-provider="dev"]')).toBeVisible();
  await expect(page.locator(`[data-testid="your-entity"][data-entity="${entity}"]`)).toBeVisible();
});

test("a route's canvas keeps its graph while its reads fail, says BEHIND, and catches up once they recover", { tag: "@server" }, async ({ context }) => {
  const [one, two] = [await context.newPage(), await context.newPage()];
  const id = await newRoute(one, "server", routeName("Behind"));
  await addNode(one, "decision", "First choice");
  await open(two, "server", `/routes/${id}`);
  await live(two);
  await expect(two.locator("[data-testid=node-card]")).toHaveCount(1);
  // The server cannot answer this route's reads for the page showing it.
  let failing = true;
  await two.route(`**/api/routes/${id}`, async (route) => {
    if (failing) {
      await route.fulfill({ status: 500, body: "the route could not be read" });
    } else {
      await route.continue();
    }
  });
  await addNode(one, "decision", "Second choice");
  await expect(syncChip(two)).toHaveAttribute("data-state", "behind");
  // What it showed stays: the canvas is not replaced by an error.
  await expect(two.locator("[data-testid=node-card]")).toHaveCount(1);
  await expect(two.getByText("The route could not be read")).toHaveCount(0);
  failing = false;
  await expect(two.locator("[data-testid=node-card]")).toHaveCount(2);
  await expect(syncChip(two)).not.toHaveAttribute("data-state", "behind");
});

test("a failed read says what happened in a sentence, and keeps the server's message, ids and all, behind Details", { tag: "@server" }, async ({ page }) => {
  const raw = "read of j_hiring at revision 4 failed in e_lead";
  await page.route((url) => url.pathname === "/api/journeys", (route) => route.fulfill({ status: 500, body: raw }));
  await open(page, "server", "/journeys");
  const failed = page.getByText("The journeys could not be read.");
  await expect(failed).toBeVisible();
  await expect(page.getByText("j_hiring")).toBeHidden();
  const details = page.getByRole("button", { name: "Details" });
  await details.click();
  await expect(page.getByTestId("failed-message")).toContainText("j_hiring");
  // Closed again: the page a person reads shows no key, which every test ends by checking.
  await details.click();
  await expect(page.getByTestId("failed-message")).toBeHidden();
});

test("C14: a proposal drafted in another page appears in a journey's Proposals tab with no reload", { tag: "@server" }, async ({ context }) => {
  const [one, two] = [await context.newPage(), await context.newPage()];
  await open(one, "server", "/entities");
  const owner = await addEntity(one, fresh("Owner"));
  const journey = await journeyOwnedBy(one.request, owner);
  await openJourney(two, "server", journey);
  await live(two);
  // Nothing is open, so the page keeps no tab for it.
  await expect(two.getByTestId("tab-proposals")).toHaveCount(0);
  const drafted = await one.request.post(`/api/journeys/${journey}/proposals`, {
    data: {
      id: `pr_${crypto.randomUUID().replaceAll("-", "")}`,
      patch_id: patchId(),
      draft: { title: "Add a follow-up", destination_revision: 1, mutations: [{ op: "add_node", node: { key: "n_followup", id: "followup", kind: "action", title: "Follow up" } }] },
    },
  });
  expect(drafted.ok(), await drafted.text()).toBe(true);
  await expect(two.getByTestId("tab-proposals").getByTestId("proposals-count")).toHaveText("1");
  await two.getByTestId("tab-proposals").click();
  await expect(two.getByTestId("proposal-row")).toContainText("Add a follow-up");
});

test("C14: the deployment's open proposals are a panel on Entities, found at its own address", { tag: "@server" }, async ({ page }) => {
  await open(page, "server", "/entities");
  const deployment = (await (await page.request.get("/api/deployment")).json()) as { revision: number };
  const drafted = await page.request.post("/api/deployment/proposals", {
    data: {
      id: `pr_${crypto.randomUUID().replaceAll("-", "")}`,
      patch_id: patchId(),
      draft: { title: "Add a reviewer", destination_revision: deployment.revision, mutations: [{ op: "create_entity", entity: { key: `e_reviewer_${crypto.randomUUID().slice(0, 8)}`, name: "Reviewer", emails: [] } }] },
    },
  });
  expect(drafted.ok(), await drafted.text()).toBe(true);
  await expect(page.getByTestId("deployment-proposals").getByTestId("proposal-row")).toContainText("Add a reviewer");
  await page.getByTestId("deployment-proposals").getByRole("link", { name: "Add a reviewer" }).click();
  await expect(page).toHaveURL(/\/entities\/proposals\/pr_[0-9a-f]+$/);
  await expect(page.getByTestId("review-trail")).toContainText("Entities");
});
