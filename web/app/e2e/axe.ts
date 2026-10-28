// axe-core's structural rules over a state (landmarks, names, roles, ARIA, headings, labels): the
// page's semantics, which no box says. Two of its rules are off because a check of ours already
// holds the same thing more strictly, from what Chrome drew: `color-contrast` (e2e/contrast.ts,
// which reads the page's own backgrounds and the done work's fade) and `target-size` (e2e/touch.ts,
// DESIGN's 44px rather than 24px, measured as the real hit area).
import type { Page } from "@playwright/test";
import axe from "axe-core";

/** The rules our own checks cover. */
const COVERED = ["color-contrast", "target-size"];

/** What axe reports for one rule: its id, what it asks for, and the elements that break it. */
interface Violation {
  id: string;
  help: string;
  nodes: { target: unknown[] }[];
}

/** The violations axe finds on `page` as it stands, one line per rule naming the first elements. */
export async function axeProblems(page: Page): Promise<string[]> {
  // eslint-disable-next-line e2e/settled-reads -- the caller has settled the page
  const loaded = await page.evaluate(() => Reflect.has(window, "axe"));
  if (!loaded) {
    await page.evaluate(axe.source);
  }
  // eslint-disable-next-line e2e/settled-reads -- as above
  const found = await page.evaluate(async (off) => {
    const axe = Reflect.get(window, "axe") as { run: (context: Document, options: object) => Promise<{ violations: Violation[] }> };
    const options = { rules: Object.fromEntries(off.map((id) => [id, { enabled: false }])), resultTypes: ["violations"] };
    return (await axe.run(document, options)).violations.map(({ id, help, nodes }) => ({ id, help, nodes: nodes.map(({ target }) => target) }));
  }, COVERED);
  return found.map(({ id, help, nodes }) => `${id}: ${help} (${nodes.slice(0, 3).map((target) => String(target)).join("; ")}${nodes.length > 3 ? `; and ${String(nodes.length - 3)} more` : ""})`);
}
