// The `test` every browser spec imports instead of Playwright's: the same, plus the invariants
// every page a test opened must hold once the test body has finished: nothing spills out of its
// box (e2e/containment.ts), no key is shown where a name belongs (e2e/internal-ids.ts), the page
// does not scroll sideways and nothing else breaks a structural rule (e2e/structure.ts), and the
// only tooltip is the app's (e2e/tips.ts).
// eslint forbids importing `test` from `@playwright/test` in this directory, so a spec cannot
// skip them.
import { expect, test as base } from "@playwright/test";

import { checkpoint } from "./invariants.ts";

export { expect };

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
