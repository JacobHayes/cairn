// The `test` every browser spec imports instead of Playwright's: the same, plus the invariants
// every page a test opened must hold once the test body has finished: nothing spills out of its
// box (e2e/containment.ts) and no key is shown where a name belongs (e2e/internal-ids.ts).
// eslint forbids importing `test` from `@playwright/test` in this directory, so a spec cannot
// skip them.
import { expect, test as base } from "@playwright/test";

import { expectContained } from "./containment.ts";
import { expectNoKeysShown } from "./internal-ids.ts";

export { expect };

export const test = base.extend<{ invariants: undefined }>({
  invariants: [
    async ({ context }, use) => {
      await use(undefined);
      for (const page of context.pages()) {
        await expectContained(page);
        await expectNoKeysShown(page);
      }
    },
    { auto: true },
  ],
});
