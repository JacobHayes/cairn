// The invariant that no screen shows a key where a name belongs: a user, entity, node, journey,
// proposal or patch id, an agent or conversation id, a digest, or a revision number, in text a
// person reads or in a tooltip or label. Every browser test ends by checking the pages it
// opened (e2e/test.ts), so a screen that prints one fails whatever it was testing. The patterns
// are the API's key shapes (the OpenAPI document's id patterns).
import type { Page } from "@playwright/test";

/** One kind of key, and the pattern that finds it. */
const KEYS: { what: string; pattern: RegExp }[] = [
  { what: "an id", pattern: /\b(?:a|ag|cv|e|i|j|k|n|p|pr|r|u)_[a-z0-9][a-z0-9_-]*\b/g },
  { what: "a digest", pattern: /\b[0-9a-f]{32,}\b/g },
  { what: "a revision number", pattern: /\brev(?:ision)?s?\.? ?#?\d+/gi },
];

/** Where a key was shown. */
export interface ShownKey {
  what: string;
  key: string;
  /** The text it sits in, cut around it. */
  around: string;
  /** Where on the page: visible text, or the attribute (title, aria-label, placeholder, alt) it is in. */
  where: string;
}

/** The text a person can read on `page`, and the attributes they can hover or hear, with every key found in them. */
export async function shownKeys(page: Page): Promise<ShownKey[]> {
  const shown = await page.evaluate(() => {
    const found: { where: string; text: string }[] = [{ where: "text", text: document.body.innerText }];
    for (const element of document.body.querySelectorAll("[title], [aria-label], [placeholder], [alt]")) {
      const style = getComputedStyle(element);
      if (style.display === "none" || style.visibility === "hidden") {
        continue;
      }
      for (const attribute of ["title", "aria-label", "placeholder", "alt"]) {
        const text = element.getAttribute(attribute);
        if (text !== null) {
          found.push({ where: attribute, text });
        }
      }
    }
    return found;
  });
  return shown.flatMap(({ where, text }) =>
    KEYS.flatMap(({ what, pattern }) =>
      [...text.matchAll(pattern)].map((match) => ({
        what,
        key: match[0],
        where,
        around: text.slice(Math.max(0, match.index - 30), match.index + match[0].length + 30).replaceAll("\n", " "),
      })),
    ),
  );
}

/** Fails when `page` shows a key where a name belongs, saying which, where, and in what text. */
export async function expectNoKeysShown(page: Page): Promise<void> {
  if (page.isClosed()) {
    return;
  }
  const keys = await shownKeys(page);
  if (keys.length > 0) {
    const lines = keys.map((each) => `${each.what} ${each.key} in ${each.where}: ...${each.around}...`);
    throw new Error(`${page.url()} shows a key where a name belongs:\n${lines.join("\n")}`);
  }
}
