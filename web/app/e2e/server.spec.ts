// The shell on the server host, against `cairn demo` (the `server` project of
// playwright.config.ts): what only a live server shows. Writes from different pages meet
// through the safe retry and the conflict (H5), a later page and a page on the index follow
// what another wrote (H6), a deployment tick re-derives what it moved, and an identity
// signed in at the demo's stub issuer links to the user (H3). Each test makes what it
// changes, so the fixtures other tests read stay as seeded. The tests tagged @serial change what
// the whole deployment holds (its identities, its revision, the scripted assistant) and run
// one at a time, after the others have finished; the rest each work on a journey or route of
// their own (ownJourney, newRoute) and run side by side.
import type { APIRequestContext, Page } from "@playwright/test";

import { checkpoint } from "./invariants.ts";
import { expect, test } from "./test.ts";

import { addIdentity, identityRow, mergeIdentities } from "./around.ts";
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
  ownJourney,
  rename,
  renameOf,
  save,
  startRename,
  syncChip,
  whenSettled,
} from "./shell.ts";

const patchId = () => `p_${crypto.randomUUID().replaceAll("-", "")}`;

const title = (page: Parameters<typeof nodeCard>[0], node: string) => nodeCard(page, node).getByTestId("title");

test("edits to different nodes both land, and edits to one field surface a conflict", { tag: "@server" }, async ({ context }) => {
  const [one, two] = [await context.newPage(), await context.newPage()];
  const { id } = await ownJourney(context.request, "hiring-loop");
  await openJourney(one, "server", id);
  await openJourney(two, "server", id);
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
  const [theirs, mine, later] = [fresh("Theirs"), fresh("Mine"), fresh("Later")];
  await startRename(two, "n_onsite", mine);
  await rename(one, "n_onsite", theirs);
  await save(two, "n_onsite");
  await expect(two.locator('[data-testid="refused"][data-rejection="stale"]')).toContainText(theirs);
  await expect(syncChip(two)).toHaveAttribute("aria-label", /^CONFLICT · 1\./);
  await expect(renameOf(two, "n_onsite").getByRole("textbox")).toHaveValue(mine);
  await expect(title(one, "n_onsite")).toHaveText(theirs);
  // What is typed after the conflict is kept across a reload.
  await renameOf(two, "n_onsite").getByRole("textbox").fill(later);
  await two.reload();
  await expect(two.locator('[data-testid="refused"][data-rejection="stale"]')).toBeVisible();
  await expect(renameOf(two, "n_onsite").getByRole("textbox")).toHaveValue(later);
  await two.getByRole("button", { name: "Keep my edit on the current version" }).click();
  await save(two, "n_onsite");
  await expect(title(two, "n_onsite")).toHaveText(later);
  await expect(title(one, "n_onsite")).toHaveText(later);
  await expect(renameOf(two, "n_onsite")).toHaveCount(0);
});

// Serial: it counts the page's fetches of the document, and a deployment tick caused by another test's journey would add one.
test("a later page shows the edit, and a page on the index shows a journey started elsewhere", { tag: ["@server", "@serial"] }, async ({ context }) => {
  const [one, two] = [await context.newPage(), await context.newPage()];
  const { id, name } = await ownJourney(context.request, "product-launch");
  const fetched = countDocumentFetches(two, id);
  await openJourney(one, "server", id);
  await openJourney(two, "server", id);
  const renamed = fresh("Docs");
  await rename(one, "n_docs", renamed);
  await expect(title(two, "n_docs")).toHaveText(renamed);
  await two.getByRole("link", { name: "Journeys" }).click();
  await expect(two.getByRole("link", { name })).toBeVisible();
  // The index is live: a journey started in the other page is listed with no reload.
  const started = await ownJourney(one.request, "vendor-evaluation");
  await expect(two.locator(`[data-testid="journey-row"][data-journey="${started.id}"]`)).toBeVisible();
  await two.getByRole("link", { name }).click();
  await goTo(two, "graph");
  await expect(title(two, "n_docs")).toHaveText(renamed);
  await live(two);
  const later = await context.newPage();
  await openJourney(later, "server", id);
  await expect(title(later, "n_docs")).toHaveText(renamed);
  // The page refetched once per newer revision, never for one it already held.
  expect(fetched()).toBe(2);
  // A proposal drafted in another page appears in the Proposals tab with no reload.
  await expect(two.getByTestId("tab-proposals")).toHaveCount(0);
  const drafted = await one.request.post(`/api/journeys/${id}/proposals`, {
    data: {
      id: `pr_${crypto.randomUUID().replaceAll("-", "")}`,
      patch_id: patchId(),
      draft: { title: "Add a follow-up", destination_revision: await derivedRevision(two), mutations: [{ op: "add_node", node: { key: "n_followup", id: "followup", kind: "action", title: "Follow up" } }] },
    },
  });
  expect(drafted.ok(), await drafted.text()).toBe(true);
  await expect(two.getByTestId("tab-proposals").getByTestId("proposals-count")).toHaveText("1");
  await two.getByTestId("tab-proposals").click();
  await expect(two.getByTestId("proposal-row")).toContainText("Add a follow-up");
});

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

test("a deployment tick refetches the deployment context and re-derives: a merge changes the owner another page shows", { tag: ["@server", "@serial"] }, async ({ context }) => {
  const [one, two] = [await context.newPage(), await context.newPage()];
  const deploymentFetches: string[] = [];
  two.on("request", (request) => {
    if (new URL(request.url()).pathname === "/api/deployment") {
      deploymentFetches.push(request.url());
    }
  });
  await open(one, "server", "/identities");
  const first = await addIdentity(one, fresh("Owner"));
  const second = await addIdentity(one, fresh("Survivor"));
  const journey = await journeyOwnedBy(one.request, first);
  await openJourney(two, "server", journey);
  await live(two);
  const survivor = (await whenSettled(one, () => identityRow(one, second).getByTestId("identity-name").textContent())) ?? "";
  // The card names an owner only when it is the viewer or missing: the node's detail names this one.
  const owners = await section(await openFromCanvas(two, "n_task"), "participations");
  await expect(owners).not.toContainText(survivor);
  const [before, fetchedBefore] = [await derivedRevision(two), deploymentFetches.length];
  await mergeIdentities(one, second, first);
  const deployment = (await (await one.request.get("/api/deployment")).json()) as { revision: number };
  await expect(syncChip(two)).toHaveAttribute("data-deployment", String(deployment.revision));
  await expect(owners).toContainText(survivor);
  expect(await derivedRevision(two)).toBe(before);
  expect(deploymentFetches.length).toBe(fetchedBefore + 1);
});

/** An identity with no name is asked for its name on load, and Save names it everywhere it shows: the dev user's own, which is the first of theirs until the sign-in leg gives them a second. */
async function nameless(page: Page): Promise<void> {
  const key = "who_k3d7qm2xw5tn6pfa";
  await open(page, "server", `/identities/${key}`);
  const detail = page.getByTestId("identity-detail");
  await expect(detail.getByTestId("yours")).toBeVisible();
  const name = (await whenSettled(page, () => detail.getByTestId("detail-title").textContent())) ?? "";
  await detail.getByRole("button", { name: "Edit" }).click();
  await detail.getByLabel("Name").fill("");
  await detail.getByRole("button", { name: "Save" }).click();
  await expect(detail.getByTestId("detail-title")).toContainText("@");
  await page.reload();
  const prompt = page.getByTestId("name-prompt");
  await expect(prompt).toBeVisible();
  await prompt.getByRole("button", { name: "Later" }).click();
  await expect(prompt).toHaveCount(0);
  await page.reload();
  await prompt.getByLabel("Name").fill(name);
  await prompt.getByRole("button", { name: "Save" }).click();
  await expect(prompt).toHaveCount(0);
  await expect(page.getByTestId("identity-detail").getByTestId("detail-title")).toHaveText(name);
  await expect(page.locator(`[data-testid="identity"][data-identity="${key}"]`).getByTestId("identity-name")).toHaveText(name);
  await page.getByRole("navigation", { name: "Screens", exact: true }).getByRole("link", { name: "You" }).click();
  await expect(page.getByTestId("your-identity").first()).toContainText(name);
}

test("a nameless identity of the user is asked for its name; an identity signed in with the stub issuer links to the user, and its verified email names their identity", { tag: ["@server", "@serial"] }, async ({ page }) => {
  await nameless(page);
  const email = `${fresh("linked").replace(/\W/g, "-")}@example.org`;
  await open(page, "server", "/identities");
  const identity = await addIdentity(page, fresh("Linked person"), [email]);
  await open(page, "server", "/me");
  await expect(page.getByTestId("login")).toHaveCount(1);
  await expect(page.locator(`[data-testid="your-identity"][data-identity="${identity}"]`)).toHaveCount(0);
  await page.getByTestId("link-login").getByRole("link", { name: "Sign in with stub" }).click();
  await page.getByLabel("Email").fill(email);
  await page.getByRole("button", { name: "Sign in" }).click();
  await expect(page.getByTestId("you-screen")).toBeVisible();
  await expect(page.locator('[data-testid="login"][data-provider="stub"]').getByTestId("login-emails")).toContainText(email);
  await expect(page.locator('[data-testid="login"][data-provider="dev"]')).toBeVisible();
  await expect(page.locator(`[data-testid="your-identity"][data-identity="${identity}"]`)).toBeVisible();
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

test("a failed read or a refused write says what happened in a sentence, and keeps the server's message, ids and all, behind Details", { tag: "@server" }, async ({ page, context }) => {
  const raw = "read of j_hiring at revision 4 failed in who_lead";
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
  // A write the server refuses says the same, on a journey of its own.
  const { id } = await ownJourney(context.request, "hiring-loop");
  await openJourney(page, "server", id);
  await page.route("**/api/journeys/*/patches", (route) => route.fulfill({ status: 400, contentType: "text/plain", body: "patch p_abc at revision 4 refused for n_offer" }));
  await startRename(page, "n_offer", fresh("Refused"));
  await save(page, "n_offer");
  const problem = page.getByTestId("write-problem").first();
  await expect(problem).toContainText("Not saved.");
  await expect(problem).not.toContainText("n_offer");
  await problem.getByRole("button", { name: "Details" }).click();
  await expect(problem.getByTestId("failed-message")).toContainText("n_offer");
  await problem.getByRole("button", { name: "Details" }).click();
  await expect(problem.getByTestId("failed-message")).toBeHidden();
});

test("C14: the deployment's open proposals are the Identities inspector's Proposals tab, found at its own address", { tag: ["@server", "@serial"] }, async ({ page }) => {
  await open(page, "server", "/identities");
  const deployment = (await (await page.request.get("/api/deployment")).json()) as { revision: number };
  const drafted = await page.request.post("/api/deployment/proposals", {
    data: {
      id: `pr_${crypto.randomUUID().replaceAll("-", "")}`,
      patch_id: patchId(),
      draft: { title: "Add a reviewer", destination_revision: deployment.revision, mutations: [{ op: "create_identity", identity: { key: `who_${Array.from(crypto.getRandomValues(new Uint8Array(16)), (byte) => "abcdefghijklmnopqrstuvwxyz234567"[byte % 32]).join("")}`, name: "Nina Alvarez", emails: [] } }] },
    },
  });
  expect(drafted.ok(), await drafted.text()).toBe(true);
  await page.getByTestId("tab-proposals").click();
  await expect(page.getByTestId("proposals-pane").getByTestId("proposal-row")).toContainText("Add a reviewer");
  await page.getByTestId("proposals-pane").getByRole("link", { name: "Add a reviewer" }).click();
  await expect(page).toHaveURL(/\/identities\/proposals\/pr_[0-9a-f]+$/);
  // The identities page, with the new identity marked as added in place.
  await expect(page.getByTestId("layer-band")).toHaveAttribute("data-mode", "reviewing");
  await expect(page.locator('[data-testid="identity"][data-diff="added"]')).toContainText("Nina Alvarez");
});

test("C14, G5: a proposal's checklist editor keeps an item typed and not added across a reload, and says saving drops a ticked item's tick", { tag: "@server" }, async ({ page, context }) => {
  const { id } = await ownJourney(context.request, "hiring-loop");
  await openJourney(page, "server", id);
  // The screen has two items and the first is ticked.
  const written = await context.request.post(`/api/journeys/${id}/patches`, {
    data: {
      patch: {
        id: patchId(),
        target: { journey: id },
        base_revision: await derivedRevision(page),
        mutations: [
          { op: "set_node_field", node: "n_screen", value: { checklist: [{ id: "alpha", title: "Alpha" }, { id: "beta", title: "Beta" }] } },
          { op: "set_ticked", node: "n_screen", item: "alpha", ticked: true },
        ],
      },
    },
  });
  expect(written.ok(), await written.text()).toBe(true);
  await page.reload();
  const proposal = `pr_${crypto.randomUUID().replaceAll("-", "")}`;
  const drafted = await context.request.post(`/api/journeys/${id}/proposals`, {
    data: {
      id: proposal,
      patch_id: patchId(),
      draft: { title: "Trim the checklist", destination_revision: await derivedRevision(page), mutations: [{ op: "set_node_field", node: "n_screen", value: { checklist: [{ id: "beta", title: "Beta" }] } }] },
    },
  });
  expect(drafted.ok(), await drafted.text()).toBe(true);
  const review = `/journeys/${id}/proposals/${proposal}/nodes/n_screen?view=graph`;
  await open(page, "server", review);
  const changes = page.getByTestId("item-changes");
  await changes.getByRole("button", { name: "Edit" }).click();
  // The saved tick on the item the proposal drops is said, as in the node form.
  await expect(changes.getByTestId("tick-dropped")).toContainText("Alpha");
  await checkpoint(page);
  await changes.getByLabel("New item", { exact: true }).fill("Typed and not added");
  await page.reload();
  await changes.getByRole("button", { name: "Edit" }).click();
  await expect(changes.getByLabel("New item", { exact: true })).toHaveValue("Typed and not added");
});

test("Connect an agent shows this server's address, makes a token that acts as its agent until it is revoked, and is one tap away on a phone", { tag: "@server" }, async ({ page, context, playwright, baseURL }) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await open(page, "server", "/");
  await page.getByRole("navigation", { name: "Screens", exact: true }).getByRole("link", { name: "Connect an agent" }).click();
  await expect(page.getByRole("heading", { name: "Connect an agent" })).toBeVisible();
  // The address is the one the server says, and Copy puts exactly it on the clipboard.
  const capabilities = (await (await context.request.get("/api/capabilities")).json()) as { mcp: { url: string } };
  const steps = page.getByTestId("client-steps");
  await expect(steps.locator("pre").first()).toContainText(`claude mcp add --transport http --scope user cairn ${capabilities.mcp.url}`);
  await page.getByRole("radio", { name: "Codex" }).check();
  await expect(steps.locator("pre").first()).toContainText(`codex mcp add cairn --url ${capabilities.mcp.url}`);
  // Another client takes the address itself, and Copy puts exactly it on the clipboard.
  await page.getByRole("radio", { name: "Other agent" }).check();
  await expect(steps.locator("pre").first()).toHaveText(capabilities.mcp.url);
  await page.getByRole("button", { name: "Copy the address" }).click();
  await expect(page.getByRole("button", { name: "Copy the address" })).toHaveText("Copied");
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(capabilities.mcp.url);
  await page.getByRole("radio", { name: "Claude Code" }).check();

  // A token is shown once, and acts as its agent for the person when it is the only credential.
  const name = fresh("Nightly");
  const panel = page.getByTestId("agent-tokens");
  await panel.getByRole("button", { name: "New token" }).click();
  await panel.getByLabel("Token name").fill(name);
  await panel.getByRole("button", { name: "Make token" }).click();
  const minted = page.getByTestId("minted-token");
  await expect(minted).toContainText("Copy this token now");
  await expect(panel.getByRole("button", { name: "New token" })).toHaveCount(0);
  await page.getByRole("radio", { name: "Other agent" }).check();
  await expect(minted.locator("pre").first()).toContainText("cairn_agent_");
  const token = (await whenSettled(page, () => minted.locator("pre").first().innerText())).trim();
  const bare = await playwright.request.newContext({ baseURL: baseURL ?? "" });
  const asAgent = await bare.get("/api/users/me", { headers: { authorization: `Bearer ${token}` } });
  const asPerson = await bare.get("/api/users/me");
  expect(asAgent.status()).toBe(200);
  const [agent, person] = [(await asAgent.json()) as { user: string; agent: string | null }, (await asPerson.json()) as { user: string; agent: string | null }];
  expect(agent.agent ?? null).not.toBeNull();
  expect(person.agent ?? null).toBeNull();
  expect(agent.user).toBe(person.user);
  await checkpoint(page);

  // Hidden, and after a reload, the token is listed and its secret is gone.
  await page.getByRole("button", { name: "Hide token" }).click();
  await expect(minted).toHaveCount(0);
  await page.reload();
  const row = panel.getByTestId("agent-token").filter({ hasText: name });
  await expect(row).toBeVisible();
  await expect(page.getByText(token)).toHaveCount(0);
  // Revoking removes the row and the token stops signing in as its agent.
  await row.getByRole("button", { name: `Revoke ${name}` }).click();
  await expect(row).toHaveCount(0);
  await expect.poll(async () => (await bare.get("/api/users/me", { headers: { authorization: `Bearer ${token}` } })).status()).toBe(401);
  await bare.dispose();

  // On a phone the rail is a top bar of four screens: the page is a link in the sync popover.
  await page.setViewportSize({ width: 390, height: 844 });
  await open(page, "server", "/journeys");
  await expect(page.getByRole("navigation", { name: "Screens", exact: true }).getByRole("link", { name: "Connect an agent" })).toBeHidden();
  await syncChip(page).click();
  await page.getByTestId("sync-popover").getByRole("link", { name: /Connect an agent/ }).click();
  await expect(page.getByRole("heading", { name: "Connect an agent" })).toBeVisible();
  await checkpoint(page);
});
