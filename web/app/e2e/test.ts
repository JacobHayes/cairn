// The `test` every browser spec imports instead of Playwright's: the same, plus the invariants
// every page a test opened must hold once the test body has finished: nothing spills out of its
// box (e2e/containment.ts) and no key is shown where a name belongs (e2e/internal-ids.ts).
// eslint forbids importing `test` from `@playwright/test` in this directory, so a spec cannot
// skip them.
import { expect, test as base, type Page } from "@playwright/test";

import { expectContained } from "./containment.ts";
import { expectNoKeysShown } from "./internal-ids.ts";

export { expect };

/**
 * The invariants (containment, no internal key shown) on `page` as it is now. They run once at the
 * end of a test body, so a test that walks through several states, each one a test of its own
 * before they were merged, calls this at the ones that are not its end: a state that spills
 * out of its box, or shows a key where a name belongs, fails where it was reached.
 */
export async function checkpoint(page: Page): Promise<void> {
  await expectContained(page);
  await expectNoKeysShown(page);
}

export const test = base.extend<{ invariants: undefined }>({
  invariants: [
    async ({ context }, use) => {
      await use(undefined);
      for (const page of context.pages()) {
        await checkpoint(page);
      }
    },
    { auto: true },
  ],
});
