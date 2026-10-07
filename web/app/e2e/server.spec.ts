// The shell on the server host, against the fixture server through Vite's proxy: views stay
// current across pages (H6), writes from different pages meet through the safe retry (H5),
// a deployment tick re-derives, and a document from a newer engine stops the tab (version
// skew). The index and owner cases land with their screens (5.5). The `binary` project of
// playwright.config.ts runs this suite again against the real binary (4.7).
import { expect, test, type APIRequestContext, type Page } from "@playwright/test";

import {
  countDocumentFetches,
  derivedRevision,
  fresh,
  nodeCard,
  open,
  openJourney,
  rename,
  renameOf,
  save,
  startRename,
} from "./shell.ts";

const title = (page: Page, node: string) => nodeCard(page, node).getByTestId("title");

test("an edit in one page appears in another", async ({ context }) => {
  const [one, two] = [await context.newPage(), await context.newPage()];
  await openJourney(one, "server", "j_hiring");
  await openJourney(two, "server", "j_hiring");
  await expect(two.getByTestId("live")).toHaveAttribute("data-status", "live");
  const renamed = fresh("Close out");
  await rename(one, "n_close_out", renamed);
  await expect(one.getByTestId("notice")).toHaveAttribute("data-tone", "saved");
  await expect(title(two, "n_close_out")).toHaveText(renamed);
  expect(await derivedRevision(two)).toBe(await derivedRevision(one));
});

test("edits to different nodes both land", async ({ context }) => {
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

test("edits to one field surface a conflict", async ({ context }) => {
  const [one, two] = [await context.newPage(), await context.newPage()];
  await openJourney(one, "server", "j_hiring");
  await openJourney(two, "server", "j_hiring");
  const [theirs, mine] = [fresh("Theirs"), fresh("Mine")];
  await startRename(two, "n_onsite", mine);
  await rename(one, "n_onsite", theirs);
  await save(two, "n_onsite");
  await expect(two.getByTestId("conflict")).toContainText("n_onsite");
  await expect(renameOf(two, "n_onsite").getByRole("textbox")).toHaveValue(mine);
  await expect(title(one, "n_onsite")).toHaveText(theirs);
  await two.getByRole("button", { name: "Keep my edit on the current version" }).click();
  await save(two, "n_onsite");
  await expect(title(two, "n_onsite")).toHaveText(mine);
  await expect(title(one, "n_onsite")).toHaveText(mine);
});

test("a later page shows the edit", async ({ context }) => {
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
  await expect(title(two, "n_docs")).toHaveText(renamed);
  await expect(two.getByTestId("live")).toHaveAttribute("data-status", "live");
  const later = await context.newPage();
  await openJourney(later, "server", "j_launch");
  await expect(title(later, "n_docs")).toHaveText(renamed);
  // H6: the page refetched once per newer revision, never for one it already held.
  expect(fetched()).toBe(2);
});

/** Creates an entity through the API: a deployment patch, which moves the deployment revision. */
async function createEntity(request: APIRequestContext): Promise<number> {
  const deployment = (await (await request.get("/deployment")).json()) as { revision: number };
  const suffix = Math.random().toString(36).slice(2, 10);
  const patch = {
    id: `p_entity_${suffix}`,
    target: "deployment",
    base_revision: deployment.revision,
    mutations: [{ op: "create_entity", entity: { key: `e_probe_${suffix}`, name: `Probe ${suffix}` } }],
  };
  const response = await request.post("/deployment/patches", { data: { patch } });
  expect(response.status()).toBe(200);
  return deployment.revision + 1;
}

test("a deployment tick refetches the deployment context and re-derives", async ({ page }) => {
  const deploymentFetches: string[] = [];
  page.on("request", (request) => {
    if (new URL(request.url()).pathname === "/deployment") {
      deploymentFetches.push(request.url());
    }
  });
  await openJourney(page, "server", "j_bakeoff");
  await expect(page.getByTestId("live")).toHaveAttribute("data-status", "live");
  const before = await derivedRevision(page);
  const fetchedBefore = deploymentFetches.length;
  const revision = await createEntity(page.request);
  await expect(page.getByTestId("derivation")).toHaveAttribute("data-deployment", String(revision));
  expect(await derivedRevision(page)).toBe(before);
  expect(deploymentFetches.length).toBe(fetchedBefore + 1);
});

test("version skew stops the tab and asks for a reload, keeping unsent edits", async ({ context }) => {
  const [one, two] = [await context.newPage(), await context.newPage()];
  await openJourney(one, "server", "j_vendor_eval");
  await openJourney(two, "server", "j_vendor_eval");
  const unsent = fresh("Unsent");
  await startRename(two, "n_criteria", unsent);
  const held = await derivedRevision(two);
  await two.route("**/journeys/j_vendor_eval/document", async (route) => {
    const response = await route.fetch();
    const document = (await response.json()) as { engine_version: string };
    await route.fulfill({ response, json: { ...document, engine_version: "99.0.0" } });
  });
  await rename(one, "n_plan", fresh("Plan"));
  await expect(two.getByTestId("skew")).toBeVisible();
  await expect(renameOf(two, "n_criteria").getByRole("button", { name: "Save" })).toBeDisabled();
  expect(await derivedRevision(two)).toBe(held);
  await two.unroute("**/journeys/j_vendor_eval/document");
  await two.getByRole("button", { name: "Reload" }).click();
  await expect(two.getByTestId("derivation")).toBeVisible();
  await expect(two.getByTestId("skew")).toHaveCount(0);
  await expect(renameOf(two, "n_criteria").getByRole("textbox")).toHaveValue(unsent);
  expect(await derivedRevision(two)).toBe(await derivedRevision(one));
});

test("the server host is chosen when a server answers", async ({ page }) => {
  await page.goto("/#/");
  await expect(page.getByTestId("host")).toHaveAttribute("data-status", "server");
  await open(page, "server");
  await expect(page.getByRole("link", { name: "Hire a platform engineer" })).toBeVisible();
});

test("an address that names no journey is shown missing, and the tab stays live", async ({ context }) => {
  const [one, two] = [await context.newPage(), await context.newPage()];
  await open(two, "server", "/journeys/not-a-journey");
  await expect(two.getByTestId("journey-missing")).toBeVisible();
  await expect(two.getByTestId("live")).toHaveAttribute("data-status", "live");
  await two.getByRole("link", { name: "Journeys", exact: true }).click();
  await two.getByRole("link", { name: "Launch the reporting release" }).click();
  await openJourney(one, "server", "j_launch");
  const renamed = fresh("Announcement");
  await rename(one, "n_announcement", renamed);
  await expect(title(two, "n_announcement")).toHaveText(renamed);
});

test("typing is held while a save is in flight, so nothing typed is lost", async ({ page }) => {
  await openJourney(page, "server", "j_launch");
  let release: () => void = () => undefined;
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route("**/journeys/j_launch/patches", async (route) => {
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
  const response = await request.post(`/journeys/${id}/patches`, { data: { patch } });
  expect(response.status()).toBe(200);
  return id;
}

test("a draft follows its journey and its host, not the screen it was typed on", async ({ page }) => {
  const copy = await journeyFromHiringRoute(page.request);
  await openJourney(page, "server", copy);
  await expect(nodeCard(page, "n_offer")).toBeVisible();
  await page.goto(`/?host=server#/journeys/j_hiring`);
  await expect(page.getByTestId("derivation")).toBeVisible();
  const draft = fresh("Offer, j_hiring's draft");
  await startRename(page, "n_offer", draft);
  await page.evaluate((to) => {
    location.hash = to;
  }, `#/journeys/${copy}/nodes/n_offer`);
  await expect(page.getByTestId("journey-name")).toContainText("Hiring copy");
  await expect(renameOf(page, "n_offer").getByRole("button", { name: /^Rename/ })).toBeVisible();
  await expect(renameOf(page, "n_offer").getByRole("textbox")).toHaveCount(0);
  await page.evaluate(() => {
    location.hash = "#/journeys/j_hiring/nodes/n_offer";
  });
  await expect(renameOf(page, "n_offer").getByRole("textbox")).toHaveValue(draft);
  await page.goto(`/?host=browser#/journeys/j_hiring/nodes/n_offer`);
  await expect(page.getByTestId("derivation")).toBeVisible();
  await expect(renameOf(page, "n_offer").getByRole("button", { name: /^Rename/ })).toBeVisible();
  await expect(renameOf(page, "n_offer").getByRole("textbox")).toHaveCount(0);
});
