// The shell on the server host, against the fixture server through Vite's proxy: views stay
// current across pages (H6), writes from different pages meet through the safe retry (H5),
// a deployment tick re-derives, and a document from a newer engine stops the tab (version
// skew). The index and owner cases land with their screens (5.5); the run against the binary
// with 4.7.
import { expect, test, type APIRequestContext, type Page } from "@playwright/test";

import {
  countDocumentFetches,
  derivedRevision,
  fresh,
  nodeRow,
  open,
  openJourney,
  rename,
  save,
  startRename,
} from "./shell.ts";

const title = (page: Page, node: string) => nodeRow(page, node).getByTestId("title");

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
  await expect(nodeRow(two, "n_onsite").getByRole("textbox")).toHaveValue(mine);
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
  await expect(two.getByTestId("journey-row")).toHaveCount(4);
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
  await expect(nodeRow(two, "n_criteria").getByRole("button", { name: "Save" })).toBeDisabled();
  expect(await derivedRevision(two)).toBe(held);
  await two.unroute("**/journeys/j_vendor_eval/document");
  await two.getByRole("button", { name: "Reload" }).click();
  await expect(two.getByTestId("derivation")).toBeVisible();
  await expect(two.getByTestId("skew")).toHaveCount(0);
  await expect(nodeRow(two, "n_criteria").getByRole("textbox")).toHaveValue(unsent);
  expect(await derivedRevision(two)).toBe(await derivedRevision(one));
});

test("the server host is chosen when a server answers", async ({ page }) => {
  await page.goto("/#/");
  await expect(page.getByTestId("host")).toHaveAttribute("data-status", "server");
  await open(page, "server");
  await expect(page.getByTestId("journey-row")).toHaveCount(4);
});
