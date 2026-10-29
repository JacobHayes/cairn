// The `test` every browser spec imports instead of Playwright's: the same, plus a `cairn demo`
// of the test's own (the real binary over the fixtures in memory, the `baseURL` the page opens),
// so each test starts from the fixtures as seeded and nothing it writes reaches another test,
// and the invariants every page a test opened must hold once the test body has finished: nothing spills out of its
// box (e2e/containment.ts), no key is shown where a name belongs (e2e/internal-ids.ts), the page
// does not scroll sideways and nothing else breaks a structural rule (e2e/structure.ts), and the
// only tooltip is the app's (e2e/tips.ts).
// eslint forbids importing `test` from `@playwright/test` in this directory, so a spec cannot
// skip them.
import { expect, test as base } from "@playwright/test";

import { startNamedDemo, type Demo } from "../src/demo-server.test-support.ts";
import { checkpoint } from "./invariants.ts";

export { expect };

export const test = base.extend<{ invariants: undefined; demo: Demo }>({
  // eslint-disable-next-line no-empty-pattern -- Playwright reads the fixtures a function needs from its destructured first argument
  demo: async ({}, use) => {
    const demo = await startNamedDemo();
    await use(demo);
    await demo.stop();
  },
  baseURL: async ({ demo }, use) => {
    await use(demo.origin);
  },
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
