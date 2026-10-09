// The shell on the server host, against the real binary (the `binary` project of
// playwright.config.ts, 4.7): views stay current across pages (H6), writes from different
// pages meet through the safe retry (H5), a deployment tick re-derives, and a document from a
// newer engine stops the tab (version skew). The index and owner cases are
// around-server.spec.ts's (5.5).
import { expect, test, type APIRequestContext, type Page } from "@playwright/test";

import {
  countDocumentFetches,
  derived,
  derivedRevision,
  fresh,
  goTo,
  goWithin,
  live,
  nodeCard,
  nodePanel,
  open,
  openJourney,
  rename,
  renameOf,
  save,
  startRename,
  syncChip,
} from "./shell.ts";

const title = (page: Page, node: string) => nodeCard(page, node).getByTestId("title");

test("edits to different nodes both land", { tag: "@server" }, async ({ context }) => {
  const [one, two] = [await context.newPage(), await context.newPage()];
  await openJourney(one, "server", "j_hiring");
  await openJourney(two, "server", "j_hiring");
  const [offer, screen] = [fresh("Offer"), fresh("Screen")];
  await startRename(two, "n_screen", screen);
  await rename(one, "n_offer", offer);
  await expect(title(two, "n_offer")).toHaveText(offer);
  await save(two, "n_screen");
  await expect(title(two, "n_screen")).toHaveText(screen);
  await expect(two.getByTestId("conflict")).toHaveCount(0);
  for (const page of [one, two]) {
    await expect(title(page, "n_offer")).toHaveText(offer);
    await expect(title(page, "n_screen")).toHaveText(screen);
  }
});

test("edits to one field surface a conflict", { tag: "@server" }, async ({ context }) => {
  const [one, two] = [await context.newPage(), await context.newPage()];
  await openJourney(one, "server", "j_hiring");
  await openJourney(two, "server", "j_hiring");
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

test("a later page shows the edit", { tag: "@server" }, async ({ context }) => {
  const [one, two] = [await context.newPage(), await context.newPage()];
  const fetched = countDocumentFetches(two, "j_launch");
  await openJourney(one, "server", "j_launch");
  await openJourney(two, "server", "j_launch");
  const renamed = fresh("Docs");
  await rename(one, "n_docs", renamed);
  await expect(title(two, "n_docs")).toHaveText(renamed);
  await two.getByRole("link", { name: "Journeys" }).click();
  await expect(two.getByRole("link", { name: "Launch the reporting release" })).toBeVisible();
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

/** Creates an entity through the API: a deployment patch, which moves the deployment revision. */
async function createEntity(request: APIRequestContext): Promise<number> {
  const deployment = (await (await request.get("/api/deployment")).json()) as { revision: number };
  const suffix = Math.random().toString(36).slice(2, 10);
  const patch = {
    id: `p_entity_${suffix}`,
    target: "deployment",
    base_revision: deployment.revision,
    mutations: [{ op: "create_entity", entity: { key: `e_probe_${suffix}`, name: `Probe ${suffix}` } }],
  };
  const response = await request.post("/api/deployment/patches", { data: { patch } });
  expect(response.status()).toBe(200);
  return deployment.revision + 1;
}

test("a deployment tick refetches the deployment context and re-derives", { tag: "@server" }, async ({ page }) => {
  const deploymentFetches: string[] = [];
  page.on("request", (request) => {
    if (new URL(request.url()).pathname === "/api/deployment") {
      deploymentFetches.push(request.url());
    }
  });
  await openJourney(page, "server", "j_bakeoff");
  await live(page);
  const before = await derivedRevision(page);
  const fetchedBefore = deploymentFetches.length;
  const revision = await createEntity(page.request);
  await expect(syncChip(page)).toHaveAttribute("data-deployment", String(revision));
  expect(await derivedRevision(page)).toBe(before);
  expect(deploymentFetches.length).toBe(fetchedBefore + 1);
});

test("version skew stops the tab and asks for a reload, keeping unsent edits", { tag: "@server" }, async ({ context }) => {
  const [one, two] = [await context.newPage(), await context.newPage()];
  await openJourney(one, "server", "j_vendor_eval");
  await openJourney(two, "server", "j_vendor_eval");
  const unsent = fresh("Unsent");
  await startRename(two, "n_access", unsent);
  const held = await derivedRevision(two);
  await two.route("**/api/journeys/j_vendor_eval/document", async (route) => {
    const response = await route.fetch();
    const document = (await response.json()) as { engine_version: string };
    await route.fulfill({ response, json: { ...document, engine_version: "99.0.0" } });
  });
  await rename(one, "n_plan", fresh("Plan"));
  await expect(syncChip(two)).toHaveAttribute("data-state", "new-version");
  await expect(renameOf(two, "n_access").getByRole("button", { name: "Save" })).toBeDisabled();
  expect(await derivedRevision(two)).toBe(held);
  await two.unroute("**/api/journeys/j_vendor_eval/document");
  await syncChip(two).click();
  await derived(two);
  await expect(syncChip(two)).not.toHaveAttribute("data-state", "new-version");
  await expect(renameOf(two, "n_access").getByRole("textbox")).toHaveValue(unsent);
  expect(await derivedRevision(two)).toBe(await derivedRevision(one));
});

test("an address that names no journey is shown missing, and the tab stays live", { tag: "@server" }, async ({ context }) => {
  const [one, two] = [await context.newPage(), await context.newPage()];
  await open(two, "server", "/journeys/not-a-journey");
  await expect(two.getByTestId("journey-missing")).toBeVisible();
  await live(two);
  await two.getByRole("link", { name: "Journeys", exact: true }).click();
  await two.getByRole("link", { name: "Launch the reporting release" }).click();
  await goTo(two, "plan", "graph");
  await openJourney(one, "server", "j_launch");
  const renamed = fresh("Announcement");
  await rename(one, "n_announcement", renamed);
  await expect(title(two, "n_announcement")).toHaveText(renamed);
});

test("typing is held while a save is in flight, so nothing typed is lost", { tag: "@server" }, async ({ page }) => {
  await openJourney(page, "server", "j_launch");
  let release: () => void = () => undefined;
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route("**/api/journeys/j_launch/patches", async (route) => {
    await held;
    await route.continue();
  });
  const renamed = fresh("Beta feedback");
  await startRename(page, "n_beta_feedback", renamed);
  await save(page, "n_beta_feedback");
  await expect(renameOf(page, "n_beta_feedback").getByRole("textbox")).toBeDisabled();
  release();
  await expect(title(page, "n_beta_feedback")).toHaveText(renamed);
});

/** Creates a journey from the hiring loop's route: its nodes share the hiring journey's keys. */
async function journeyFromHiringRoute(request: APIRequestContext): Promise<string> {
  const suffix = Math.random().toString(36).slice(2, 10);
  const id = `j_copy_${suffix}`;
  const patch = {
    id: `p_copy_${suffix}`,
    target: { journey: id },
    base_revision: 0,
    mutations: [{ op: "create_journey", name: `Hiring copy ${suffix}`, from: { route: "hiring-loop", version: 1 } }],
  };
  const response = await request.post(`/api/journeys/${id}/patches`, { data: { patch } });
  expect(response.status()).toBe(200);
  return id;
}

test("a draft follows its journey, not the screen it was typed on", { tag: "@server" }, async ({ page }) => {
  const copy = await journeyFromHiringRoute(page.request);
  await openJourney(page, "server", copy);
  await expect(nodeCard(page, "n_offer")).toBeVisible();
  await open(page, "server", "/journeys/j_hiring/plan/graph");
  await derived(page);
  const draft = fresh("Offer, j_hiring's draft");
  await startRename(page, "n_offer", draft);
  await goWithin(page, `/journeys/${copy}/plan/graph/nodes/n_offer`);
  await expect(page.getByTestId("journey-name")).toContainText("Hiring copy");
  await expect(nodePanel(page, "n_offer")).toBeVisible();
  await expect(renameOf(page, "n_offer")).toHaveCount(0);
  await goWithin(page, "/journeys/j_hiring/plan/graph/nodes/n_offer");
  await expect(renameOf(page, "n_offer").getByRole("textbox")).toHaveValue(draft);
});
