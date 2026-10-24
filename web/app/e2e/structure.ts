// The structural rules (DESIGN, Typography and the rest): plain functions over the page as Chrome
// laid it out (e2e/snapshot.ts), each returning one line per problem. Every browser test is
// checked after it ends (e2e/test.ts) and a picture before it is taken (e2e/invariants.ts), so a
// screen never opts in.
import { expect, type Page } from "@playwright/test";

import { drawnTree, type Drawn } from "./snapshot.ts";

/** The smallest text the app draws, in px: DESIGN's mono label size (`--type-label-size`), which chips and every mono label are set in. */
export const TYPE_FLOOR_PX = 12;

/** `el` or an ancestor has one of `classes`. */
function inClass(el: Drawn | undefined, classes: readonly string[]): boolean {
  for (let up = el; up !== undefined; up = up.parent) {
    const names = (up.attr["class"] ?? "").split(/\s+/);
    if (classes.some((each) => names.includes(each))) {
      return true;
    }
  }
  return false;
}

/** A container's title below the near zoom is sized to the room its head has (DESIGN, Graph cards), which may be under the floor: it carries `--title-fit`. */
function shrunkToFit(run: Drawn): boolean {
  for (let up: Drawn | undefined = run; up !== undefined; up = up.parent) {
    if ((up.attr["style"] ?? "").includes("--title-fit")) {
      return true;
    }
  }
  return false;
}

/** What a run of text is, for the report. */
function roleOf(run: Drawn): "chip" | "mono label" | "text" {
  if (inClass(run.parent, ["chip", "chip-value", "badge", "status-chip"])) {
    return "chip";
  }
  return run.style["font-family"].includes("Mono") ? "mono label" : "text";
}

/** The drawn text under the floor, one line per element and size: its role, size and words. */
export function textUnderFloor(tree: readonly Drawn[]): string[] {
  const found = new Map<string, string>();
  for (const run of tree) {
    const size = Number.parseFloat(run.style["font-size"]);
    const words = (run.text ?? "").trim().replace(/\s+/g, " ");
    if (run.text === undefined || words === "" || size >= TYPE_FLOOR_PX || run.style.visibility !== "visible" || run.width === 0 || shrunkToFit(run)) {
      continue;
    }
    const owner = run.parent;
    const where = `${owner?.tag ?? ""}${(owner?.attr["class"] ?? "") === "" ? "" : `.${(owner?.attr["class"] ?? "").split(/\s+/).join(".")}`}`;
    const key = `${roleOf(run)} ${where} ${String(size)}`;
    if (!found.has(key)) {
      found.set(key, `${roleOf(run)} "${words.slice(0, 30)}" in <${where}> is ${String(size)}px, under the ${String(TYPE_FLOOR_PX)}px floor`);
    }
  }
  return [...found.values()];
}

/** Fails, listing every problem, when `page` as it stands breaks a structural rule. */
export async function expectStructure(page: Page, where = "the page"): Promise<void> {
  if (page.isClosed() || page.url() === "about:blank") {
    return;
  }
  const tree = await drawnTree(page);
  expect([...textUnderFloor(tree)], `structure of ${where}`).toEqual([]);
}
