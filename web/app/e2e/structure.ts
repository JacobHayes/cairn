// The structural rules (DESIGN, Typography): plain functions over the page as Chrome
// laid it out (e2e/snapshot.ts), each returning one line per problem. Every browser test is
// checked after it ends (e2e/test.ts) and a picture before it is taken (e2e/invariants.ts), so a
// screen never opts in. `observe` reads the page once and runs every rule over it; the result also
// says what came near a rule, which `capture` saves beside the picture.
import { expect, type Page } from "@playwright/test";

import { containmentProblems, ellipsised } from "./containment.ts";
import { contrastReport } from "./contrast.ts";
import { edgeProblems, pageHeightProblems } from "./edges.ts";
import { keyProblems } from "./internal-ids.ts";
import { classesOf, snapshotOf, upFrom, type Part, type Snapshot } from "./snapshot.ts";
import { titleProblems } from "./tips.ts";
import { touchReport } from "./touch.ts";


/** A container's title below the near zoom is sized to the room its head has (DESIGN, Graph cards), which may be under the floor: it carries `--title-fit`. */
function shrunkToFit(run: Part): boolean {
  return [...upFrom(run)].some((up) => (up.attr["style"] ?? "").includes("--title-fit"));
}

/** What a run of text is, for the report. */
function roleOf(run: Part): "chip" | "mono label" | "text" {
  if ([...upFrom(run.parent)].some((up) => classesOf(up).some((each) => ["chip", "chip-value", "badge", "status-chip"].includes(each)))) {
    return "chip";
  }
  return (run.box?.style["font-family"] ?? "").includes("Mono") ? "mono label" : "text";
}

/** The drawn text runs and generated words with their size, in px. */
function textRuns(snapshot: Snapshot): { run: Part; size: number; words: string }[] {
  return snapshot.nodes.flatMap((run) => {
    const words = (run.text ?? "").trim().replace(/\s+/g, " ");
    const box = run.box;
    return box === undefined || words === "" || box.style.visibility !== "visible" || box.width === 0 ? [] : [{ run, size: Number.parseFloat(box.style["font-size"]), words }];
  });
}

/** The drawn text under `floor` px (DESIGN's mono label size, `--type-label-size`: what chips and every mono label are set in),  one line per element and size: its role, size and words. */
export function textUnderFloor(snapshot: Snapshot, floor: number): string[] {
  const found = new Map<string, string>();
  for (const { run, size, words } of textRuns(snapshot)) {
    if (size >= floor || shrunkToFit(run)) {
      continue;
    }
    const owner = run.parent;
    const classes = owner === undefined ? "" : classesOf(owner).join(".");
    const where = `${owner?.tag ?? ""}${classes === "" ? "" : `.${classes}`}`;
    const key = `${roleOf(run)} ${where} ${String(size)}`;
    if (!found.has(key)) {
      found.set(key, `${roleOf(run)} "${words.slice(0, 30)}" in <${where}> is ${String(size)}px, under the ${String(floor)}px floor`);
    }
  }
  return [...found.values()];
}

/** The page scrolling sideways: its document is wider than its window. */
export function pageScrollProblems(snapshot: Snapshot): string[] {
  const root = snapshot.nodes.find((part) => part.tag === "html" && part.box !== undefined);
  const box = root?.box;
  return box !== undefined && box.scroll.width > box.client.width ? [`the page scrolls sideways: ${String(box.scroll.width)}px of content in a ${String(box.client.width)}px window`] : [];
}

/** What a rule found, by rule. */
export type Problems = Record<string, string[]>;

/** What came near a rule on a page that passed them: the facts a critique reads beside the picture. */
export interface Near {
  /** Text that is cut by an ellipsis or a clamp now. */
  ellipsised: string[];
  /** The five smallest hit boxes among the controls. */
  smallestHitAreas: { name: string; width: number; height: number }[];
  /** The font sizes in use, in px, with how many runs of text each sets. */
  fontSizes: Record<string, number>;
  /** The five lowest contrast ratios among the text the rule could decide. */
  lowestContrast: { words: string; ratio: number }[];
}

/** The page as it stands, against every rule, and what came near them. */
export interface Observed {
  problems: Problems;
  near: Near;
}

/** Reads `page` once and runs every structural rule over it. */
export async function observe(page: Page): Promise<Observed> {
  const snapshot = await snapshotOf(page);
  const touch = await touchReport(page, snapshot);
   
  const floor = Number.parseFloat(await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue("--type-label-size")));
  if (!(floor > 0)) {
    throw new Error("the --type-label-size token is not defined");
  }
  const snoozed = Number.parseFloat(await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue("--opacity-snoozed")));
  if (!(snoozed > 0)) {
    throw new Error("the --opacity-snoozed token is not defined");
  }
  const contrast = contrastReport(snapshot, snoozed);
  const sizes: Record<string, number> = {};
  for (const { size } of textRuns(snapshot)) {
    sizes[String(size)] = (sizes[String(size)] ?? 0) + 1;
  }
  return {
    problems: {
      "elements outside their boxes or text cut without a sign": containmentProblems(snapshot),
      "a key shown where a name belongs": keyProblems(snapshot),
      "a native tooltip": titleProblems(snapshot),
      "controls under the touch target": touch.problems,
      "the page scrolling sideways": pageScrollProblems(snapshot),
      "the page scrolling down at a desktop width": pageHeightProblems(snapshot),
      "content past the window's edge": edgeProblems(snapshot),
      "text under the contrast floor": contrast.problems,
      "text under the type floor": textUnderFloor(snapshot, floor),
    },
    near: {
      ellipsised: ellipsised(snapshot).slice(0, 10),
      smallestHitAreas: touch.sizes.sort((a, b) => a.width * a.height - b.width * b.height).slice(0, 5),
      fontSizes: Object.fromEntries(Object.entries(sizes).sort((a, b) => Number(a[0]) - Number(b[0]))),
      lowestContrast: contrast.lowest,
    },
  };
}

/** The problems as one report naming `where`, or nothing when there are none. */
export function report(problems: Problems, where: string): string | undefined {
  const lines = Object.entries(problems).flatMap(([rule, found]) => (found.length === 0 ? [] : [`${rule}:`, ...found.map((each) => `  ${each}`)]));
  return lines.length === 0 ? undefined : `${where} breaks a structural rule:\n${lines.join("\n")}`;
}

/** Whether `page` can be read: open, and not blank. */
export function readable(page: Page): boolean {
  return !page.isClosed() && page.url() !== "about:blank";
}

/** Reads `page` and fails, listing every problem, when it breaks a structural rule. A page still settling is read again until it passes or the assertion times out. */
export async function expectStructure(page: Page, where = "the page"): Promise<Observed | undefined> {
  if (!readable(page)) {
    return undefined;
  }
  let seen: Observed | undefined;
  await expect
    .poll(async () => {
      try {
        seen = await observe(page);
        return report(seen.problems, where) ?? "";
      } catch (error) {
        // A page that navigated away while the rules ran has nothing to check.
        if (error instanceof Error && /context was destroyed|closed|Target page/.test(error.message)) {
          seen = undefined;
          return "";
        }
        throw error;
      }
    })
    .toBe("");
  return seen;
}
