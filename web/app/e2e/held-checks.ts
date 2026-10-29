// The engine's answer to a value check takes a moment on a busy tab. A test that cares what happens
// meanwhile holds the answers back (the checks are posted to the derive worker only once released),
// does what a person does in that moment, then releases them.
import type { Page } from "@playwright/test";

interface Held {
  heldChecks: boolean;
  queuedChecks: (() => void)[];
  askedChecks: number;
}

/** Installs the hold, off; call before the page opens. */
export async function allowHoldingChecks(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const held = window as unknown as Held;
    held.heldChecks = false;
    held.queuedChecks = [];
    held.askedChecks = 0;
    const proto = Worker.prototype as { postMessage: (this: Worker, message: { op?: string }, ...rest: unknown[]) => void };
    const post = proto.postMessage;
    proto.postMessage = function (this: Worker, message, ...rest) {
      if (message.op === "value_problems") {
        held.askedChecks += 1;
      }
      if (held.heldChecks && message.op === "value_problems") {
        held.queuedChecks.push(() => { post.call(this, message, ...rest); });
        return;
      }
      post.call(this, message, ...rest);
    };
  });
}

/** From now on the checks asked are held back. */
export async function holdChecks(page: Page): Promise<void> {
  await page.evaluate(() => { (window as unknown as Held).heldChecks = true; });
}

/** Lets the held checks, and any asked later, go to the engine. */
export async function releaseChecks(page: Page): Promise<void> {
  await page.evaluate(() => {
    const held = window as unknown as Held;
    held.heldChecks = false;
    for (const post of held.queuedChecks.splice(0)) {
      post();
    }
  });
}

/** How many checks the app has asked the engine since the page opened, held or not: what a test of when it asks counts. */
export async function checksAsked(page: Page): Promise<number> {
  // eslint-disable-next-line e2e/settled-reads -- a count at this moment is the test's question: callers poll it or hold the clock still
  return page.evaluate(() => (window as unknown as Held).askedChecks);
}
