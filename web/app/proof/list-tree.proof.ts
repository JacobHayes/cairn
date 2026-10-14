// The Plan list's tree with every level open, in light and dark, written to CAIRN_PROOF_OUT:
// connected guide lines, fold chevrons, and leaf bullets that line titles up by level.
import { join } from "node:path";

import { expect, test } from "@playwright/test";

import { openActing } from "../e2e/acting.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";

test.use({ viewport: { width: 1200, height: 900 } });

for (const scheme of ["light", "dark"] as const) {
  test(`the list tree, ${scheme}`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await openActing(page, "browser", "j_hiring", "list");
    await page.getByTestId("list-fold-all").click();
    await expect(page.getByTestId("list-fold").first()).toHaveAttribute("aria-expanded", "true");
    await page.screenshot({ path: join(out, `list-tree-${scheme}.png`) });
  });
}
