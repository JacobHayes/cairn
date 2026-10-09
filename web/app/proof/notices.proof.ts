// The proof's media for brief 7.5 (briefs/proof/7.5/prove.sh): a screenshot of each state of
// notices (the draft listing one, the same draft after an edge clears it, an import's entry in the
// sync chip's Recent), written to CAIRN_PROOF_OUT. Each step asserts what its picture shows. The in-browser
// host, seeded on each load.
import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

import { savedText } from "../e2e/around.ts";
import { addNode, formField, newRoute, pickByTitle, routeName, saveForm, section, structure } from "../e2e/authoring.ts";
import { nodeCard, open, syncChip } from "../e2e/shell.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
const shot = (page: Page, name: string) => page.screenshot({ path: join(out, `${name}.png`) });

test.use({ viewport: { width: 1000, height: 640 } });

test("a draft lists work the final milestone cannot see, an edge clears it, and an import lists it too", async ({ page }) => {
  await newRoute(page, "browser", routeName("Notices"));
  const report = await addNode(page, "deliverable", "Final report");
  await addNode(page, "milestone", "Launch");
  await formField(page, "final").getByRole("checkbox").check();
  await saveForm(page);
  const edges = await section(page, "author-edges");
  await pickByTitle(edges.getByLabel("Require"), "Final report");
  await edges.getByRole("button", { name: "Add the requirement" }).click();
  await addNode(page, "deliverable", "Handbook");
  const notices = page.getByTestId("route-notices");
  await expect(notices.getByTestId("route-notice")).toHaveCount(1);
  await shot(page, "1-the-draft-lists-its-notice");

  await nodeCard(page, report).getByTestId("card-open").click();
  await expect(structure(page)).toHaveAttribute("data-node", report);
  const requirements = await section(page, "author-edges");
  await pickByTitle(requirements.getByLabel("Require"), "Handbook");
  await requirements.getByRole("button", { name: "Add the requirement" }).click();
  await expect(notices).toHaveCount(0);
  await shot(page, "2-the-edge-clears-it");

  await open(page, "browser", "/routes/vendor-evaluation/versions");
  const version = page.locator('[data-testid="version"][data-version="1"]');
  const exported = await savedText(page, () => version.getByRole("button", { name: "Export" }).click());
  await page.getByTestId("route-actions").getByLabel("Import a file as a new draft").setInputFiles({ name: "vendor-evaluation.yaml", mimeType: "text/yaml", buffer: Buffer.from(exported) });
  await syncChip(page).click();
  const saved = page.getByTestId("sync-recent").filter({ hasText: "Imported" });
  await expect(saved).toContainText("No chain to the final milestone");
  await expect(saved).toContainText("purpose, setup/workload");
  await saved.screenshot({ path: join(out, "3-an-import-lists-its-notices.png") });
});
