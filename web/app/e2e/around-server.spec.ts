// Brief 5.5 on the server host, against the fixture server through Vite's proxy: views stay
// current across pages (H6) when a journey is started (the index) and when two entities are
// merged (an owner on a journey's canvas), and an identity signed in with 3.2's stub issuer
// links to the signed-in user, whose verified email then names their entity (H3). Each test
// makes what it changes, so the fixtures other tests read stay as seeded.
import { expect, test, type APIRequestContext } from "@playwright/test";

import { addEntity, journeyName, mergeEntities, startJourney } from "./around.ts";
import { fresh, nodeCard, open, openJourney } from "./shell.ts";

const patchId = () => `p_${crypto.randomUUID().replaceAll("-", "")}`;

async function post(request: APIRequestContext, path: string, patch: object): Promise<void> {
  const response = await request.post(path, { data: { patch } });
  expect(response.ok(), await response.text()).toBe(true);
}

/** An empty journey with one action owned by `owner`, made through the API; its id. */
async function journeyOwnedBy(request: APIRequestContext, owner: string): Promise<string> {
  const deployment = (await (await request.get("/deployment")).json()) as { revision: number };
  const id = `j_${fresh("owned").replace(/\W/g, "_").toLowerCase()}`;
  await post(request, `/journeys/${id}/patches`, {
    id: patchId(),
    target: { journey: id },
    base_revision: 0,
    deployment_revision: deployment.revision,
    mutations: [
      { op: "create_journey", name: "Owned work" },
      { op: "add_node", node: { key: "n_task", id: "task", kind: "action", title: "The task" } },
      { op: "set_participation", node: "n_task", kind: "k_owner", source: [owner] },
    ],
  });
  return id;
}

test("H6: a journey started in one page appears in another page's index", async ({ context }) => {
  const [one, two] = [await context.newPage(), await context.newPage()];
  await open(two, "server", "/");
  await expect(two.getByTestId("live")).toHaveAttribute("data-status", "live");
  await expect(two.getByTestId("journey-row").first()).toBeVisible();
  const name = journeyName("Seen elsewhere");
  const id = await startJourney(one, "server", name, { route: "product-launch" });
  await expect(two.locator(`[data-testid="journey-row"][data-journey="${id}"]`)).toContainText(name);
});

test("H6, E6: an entity merge in one page changes the owner another page shows", async ({ context }) => {
  const [one, two] = [await context.newPage(), await context.newPage()];
  await open(one, "server", "/entities");
  const first = await addEntity(one, fresh("Owner"));
  const second = await addEntity(one, fresh("Survivor"));
  const journey = await journeyOwnedBy(one.request, first);
  await openJourney(two, "server", journey);
  await expect(two.getByTestId("live")).toHaveAttribute("data-status", "live");
  const survivor = (await one.locator(`[data-testid="entity"][data-entity="${second}"]`).getByTestId("entity-name").textContent()) ?? "";
  await expect(nodeCard(two, "n_task").getByTestId("card-owner")).not.toContainText(survivor);
  await mergeEntities(one, second, first);
  await expect(nodeCard(two, "n_task").getByTestId("card-owner")).toContainText(survivor);
});

test("H3: an identity signed in with the stub issuer links to the user, and its verified email names their entity", async ({ page }) => {
  const email = `${fresh("linked").replace(/\W/g, "-")}@example.org`;
  await open(page, "server", "/entities");
  const entity = await addEntity(page, fresh("Linked person"), email);
  await open(page, "server", "/me");
  await expect(page.getByTestId("identity")).toHaveCount(1);
  await expect(page.locator(`[data-testid="your-entity"][data-entity="${entity}"]`)).toHaveCount(0);
  await page.getByTestId("link-identity").getByRole("link", { name: "Sign in with stub" }).click();
  await page.getByLabel("Subject").fill(fresh("subject").replace(/\W/g, "-"));
  await page.getByLabel("Name").fill("Linked Person");
  await page.getByLabel("Email", { exact: true }).fill(email);
  await page.getByRole("button", { name: "Sign in" }).click();
  await expect(page.getByTestId("identity-screen")).toBeVisible();
  await expect(page.locator('[data-testid="identity"][data-provider="stub"]').getByTestId("identity-emails")).toContainText(email);
  await expect(page.locator('[data-testid="identity"][data-provider="dev"]')).toBeVisible();
  await expect(page.locator(`[data-testid="your-entity"][data-entity="${entity}"]`)).toBeVisible();
});

test("H5: an entity edited in another page meanwhile is not overwritten by an edit opened before it", async ({ context }) => {
  const [one, two] = [await context.newPage(), await context.newPage()];
  await open(one, "server", "/entities");
  const key = await addEntity(one, fresh("Edited"));
  const row = (page: typeof one) => page.locator(`[data-testid="entity"][data-entity="${key}"]`);
  await row(one).getByRole("button", { name: "Edit" }).click();
  await row(one).getByLabel("Name").fill(fresh("Mine"));
  await open(two, "server", "/entities");
  const theirs = fresh("Theirs");
  await row(two).getByRole("button", { name: "Edit" }).click();
  await row(two).getByLabel("Name").fill(theirs);
  await row(two).getByRole("button", { name: "Save" }).click();
  await expect(row(one).getByTestId("entity-name")).toHaveText(theirs);
  await row(one).getByRole("button", { name: "Save" }).click();
  await expect(row(one).getByTestId("refused")).toHaveAttribute("data-rejection", "stale");
  await expect(row(two).getByTestId("entity-name")).toHaveText(theirs);
});
