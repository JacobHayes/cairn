// The `test` every browser spec imports instead of Playwright's: the same, plus the containment
// invariant (e2e/containment.ts) checked automatically on every page the test opened, once the
// test body has finished. eslint forbids importing `test` from `@playwright/test` in this
// directory, so a spec cannot skip it.
import { expect, test as base } from "@playwright/test";

import { expectContained } from "./containment.ts";

export { expect };

export const test = base.extend<{ contained: undefined }>({
  contained: [
    async ({ context }, use) => {
      await use(undefined);
      for (const page of context.pages()) {
        await expectContained(page);
      }
    },
    { auto: true },
  ],
});
