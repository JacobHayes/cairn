// The accessibility snapshot of a screen (Playwright's `ariaSnapshot`: roles and names, as a
// screen reader meets them), trimmed to what reads like the screen: no link targets, no text
// runs, no values after a name. It is saved beside a picture for the screen critique to read.
import type { Page } from "@playwright/test";

/** `yaml` without link targets, text runs and the values that follow a name, and without the colon of a node that kept no children. */
export function trimAria(yaml: string): string {
  const kept = yaml
    .split("\n")
    .filter((line) => !/^\s*- (\/url:|text:)/.test(line))
    .map((line) => line.replace(/^(\s*- '.*?')\s*: .*$/, "$1").replace(/^(\s*- [^'"]*"[^"]*")\s*: .*$/, "$1"));
  return kept
    .map((line, at) => {
      const next = kept[at + 1] ?? "";
      const indent = (text: string) => text.length - text.trimStart().length;
      return line.endsWith(":") && indent(next) <= indent(line) ? line.slice(0, -1) : line;
    })
    .join("\n");
}

/** The trimmed accessibility snapshot of `page` as it stands. */
export async function ariaOf(page: Page): Promise<string> {
  return trimAria(await page.locator("body").ariaSnapshot());
}
