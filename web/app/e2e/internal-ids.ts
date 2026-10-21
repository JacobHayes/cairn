// The invariant that no screen shows a key where a name belongs: a user, identity, node, journey,
// proposal or patch id, an agent or conversation id, a digest, or a revision number, in text a
// person reads or in a tooltip or label; nor a requirement id or the name of one of the
// repository's documents, which a person cannot follow. Every browser test ends by checking the
// pages it opened (e2e/test.ts), so a screen that prints one fails whatever it was testing. The
// key prefixes are the engine's (generated, every key type's), the requirement ids' letters are
// the PRD's section headings, and the documents are the capitalised Markdown files at the
// repository's root, as the served-text check in the integration tests reads them.
import { readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import type { Page } from "@playwright/test";

import { KEY_PREFIXES } from "../../client/generated/rules.ts";

const ROOT = fileURLToPath(new URL("../../../", import.meta.url));

/** The letters of the PRD's requirement sections: each `### X.` heading. */
const SECTIONS = [...readFileSync(`${ROOT}PRD.md`, "utf8").matchAll(/^### ([A-Z])\. /gm)].map((match) => match[1]).join("");

/** The repository's documents: the Markdown files at its root named in capitals. */
const DOCUMENTS = readdirSync(ROOT)
  .map((name) => /^([A-Z]+)\.md$/.exec(name)?.[1])
  .filter((name) => name !== undefined);

/** One kind of key, and the pattern that finds it. */
const KEYS: { what: string; pattern: RegExp }[] = [
  { what: "an id", pattern: new RegExp(`\\b(?:${KEY_PREFIXES.join("|")})[a-z0-9][a-z0-9_-]*\\b`, "g") },
  { what: "a digest", pattern: /\b[0-9a-f]{32,}\b/g },
  { what: "a revision number", pattern: /\brev(?:ision)?s?\.? ?#?\d+/gi },
  { what: "a requirement id", pattern: new RegExp(`(?<![\\w\`])[${SECTIONS}]\\d{1,2}[a-z]?(?![\\w\`])`, "g") },
  { what: "a document name", pattern: new RegExp(`\\b(?:${DOCUMENTS.join("|")})\\b`, "g") },
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
