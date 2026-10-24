// The invariant that a page never scrolls sideways: its document is no wider than the window.
// Containment (e2e/containment.ts) holds each element to its box but ends its walk at the body,
// so the page's own width is checked here, once for every page a test opened (e2e/test.ts), not
// by each screen's test.
import { expect, type Page } from "@playwright/test";

/** Fails when `page`'s document is wider than its window. */
export async function expectFitsWindow(page: Page): Promise<void> {
  if (page.isClosed() || page.url() === "about:blank") {
    return;
  }
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth).catch(() => true), "the page scrolls sideways").toBe(true);
}
