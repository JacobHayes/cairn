// Brief 5.6 on the server host: an object edit (a resource, a role) is drafted against the
// revision its author opened it at, so a change another page made to the same object in the
// meantime is shown as a conflict rather than overwritten (H5).
import { expect, test } from "@playwright/test";

import { dismissNotices, openEditing, section } from "./authoring.ts";
import { fresh, openFromCanvas } from "./shell.ts";

test("H5: a resource edited in another page meanwhile is reported, not overwritten, and kept on the current version when asked", async ({ context }) => {
  const [one, two] = [await context.newPage(), await context.newPage()];
  for (const page of [one, two]) {
    await openEditing(page, "server", "j_launch");
    await openFromCanvas(page, "n_announcement");
  }
  const first = await section(one, "author-resources");
  await first.locator('[data-testid="resource"][data-key="a_announcement_draft"]').getByRole("button", { name: "Edit" }).click();
  const mine = fresh("Mine");
  await first.getByLabel("Resource title").fill(mine);
  const second = await section(two, "author-resources");
  await second.locator('[data-testid="resource"][data-key="a_announcement_draft"]').getByRole("button", { name: "Edit" }).click();
  const theirs = fresh("Theirs");
  await second.getByLabel("Resource title").fill(theirs);
  await second.getByRole("button", { name: "Save the resource" }).click();
  await expect(second.getByTestId("resource-form")).toHaveCount(0);
  await expect(first.locator('[data-testid="resource"][data-key="a_announcement_draft"]')).toContainText(theirs);
  await dismissNotices(one);
  await first.getByRole("button", { name: "Save the resource" }).click();
  await expect(first.getByTestId("refused")).toBeVisible();
  await expect(first.locator('[data-testid="resource"][data-key="a_announcement_draft"]')).toContainText(theirs);
  await first.getByRole("button", { name: "Keep my edit on the current version" }).click();
  await expect(first.locator('[data-testid="resource"][data-key="a_announcement_draft"]')).toContainText(mine);
});
