// The shell on the server host, against `cairn demo` (the `server` project of
// playwright.config.ts): what only a live server shows. Writes from different pages meet
// through the safe retry and the conflict (H5), a later page and a page on the index follow
// what another wrote (H6), a deployment tick re-derives what it moved, and an identity
// signed in at the demo's stub issuer links to the user (H3). Each test makes what it
// changes, so the fixtures other tests read stay as seeded.
import { expect, test, type APIRequestContext } from "@playwright/test";

import { startVendorJourney } from "./acting.ts";
import { addEntity, mergeEntities } from "./around.ts";
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
  await expect(two.getByTestId("conflict")).toHaveCount(0);
  await expect(title(one, "n_screen")).toHaveText(screen);
  // One to the same field is not: it is shown beside the other's, and kept on the current version when asked.
  const [theirs, mine] = [fresh("Theirs"), fresh("Mine")];
  await startRename(two, "n_onsite", mine);
  await rename(one, "n_onsite", theirs);
  await save(two, "n_onsite");
  await expect(two.getByTestId("conflict")).toContainText("n_onsite");
  await expect(syncChip(two)).toHaveText("CONFLICT · 1");
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
  await goTo(two, "plan", "graph");
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
  const entity = await addEntity(page, fresh("Linked person"), email);
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
