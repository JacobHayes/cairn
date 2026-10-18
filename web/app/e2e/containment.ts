// The containment invariant (DESIGN, Nothing spills out of its box): on the page as it stands,
// no visible element sticks out of the bounded box it belongs to, and no text is visibly cut
// without an ellipsis or a clamp. Every browser test is checked after it ends (e2e/test.ts), and
// a proof's capture is checked before it is taken (`capture`), so a spec never opts in.
//
// The check reads geometry only, never words. An element is checked against its nearest ancestor
// that is a bounded box (BOUNDED); a scroll container on the way excuses the axis it scrolls, and
// a canvas's viewport ends the walk (its cards are placed freely). What DESIGN lets overhang (a
// hanging tag, a rank tag, a popover, a menu, a tooltip, a connection handle) carries
// `data-overhang`: it is not held to the box it hangs from, and is itself a bounded box for what
// it holds.
import { expect, type Page } from "@playwright/test";

/** The boxes things belong to: a card, a panel, a chip or badge, a button, a list or table row, an inspector section, a toolbar, a header, and what overhangs (a popover holds its own content). */
export const BOUNDED = [
  '[data-testid="node-card"]',
  ".panel",
  ".callout",
  ".chip",
  ".badge",
  "button",
  ".button",
  "tr",
  '[role="row"]',
  ".node-row",
  ".node-panel",
  ".detail-section",
  ".toast",
  ".view-controls",
  ".ws-head",
  "header",
  "[data-overhang]",
].join(", ");

/** What the in-page steps share, installed on `window` for the length of one check. */
interface Kit {
  /** Every element the check looks at: drawn, not hidden, outside every `data-overhang` subtree. */
  seen: HTMLElement[];
  /** `el`'s nearest bounded ancestor, the axes a scroll container or a pinned (sticky or fixed) ancestor on the way excuses, and the ancestors between that clip what they hold. */
  boundOf(el: HTMLElement): { bound: HTMLElement; skip: { x: boolean; y: boolean }; clips: HTMLElement[] } | undefined;
  /** A name for the report: the element, its text and its box. */
  name(el: HTMLElement): string;
}

/** Installs the kit in the page (as `window.__containmentKit`: an in-page function cannot see this module's constants). */
function installKit(bounded: string): void {
  const all = [...document.querySelectorAll<HTMLElement>("body *")];
  // What is not drawn though a browser gives it a box: a closed disclosure's content, and what a clip-path (the visually-hidden pattern) cuts away.
  const unseen = new Map<Element, boolean>();
  const folded = (el: HTMLElement): boolean => {
    const known = unseen.get(el);
    if (known !== undefined) {
      return known;
    }
    const parent = el.parentElement;
    const closed = parent instanceof HTMLDetailsElement && !parent.open && el.tagName !== "SUMMARY";
    const result = closed || getComputedStyle(el).clipPath !== "none" || (parent !== null && parent !== document.body && folded(parent));
    unseen.set(el, result);
    return result;
  };
  const drawn = (el: HTMLElement) => {
    const box = el.getBoundingClientRect();
    const style = getComputedStyle(el);
    return box.width > 0 && box.height > 0 && !folded(el) && style.visibility === "visible" && !el.closest("svg") && !el.matches("[data-overhang]") && !el.closest(".visually-hidden, [hidden]") && !(el instanceof SVGElement);
  };
  const box = (el: HTMLElement) => {
    const at = el.getBoundingClientRect();
    return `[${[at.left, at.top, at.width, at.height].map((n) => String(Math.round(n * 10) / 10)).join(", ")}]`;
  };
  const kit: Kit = {
    seen: all.filter(drawn),
    boundOf(el) {
      const skip = { x: false, y: false };
      const clips: HTMLElement[] = [];
      for (let up: HTMLElement | null = el; up !== null && up !== document.body; up = up.parentElement) {
        const style = getComputedStyle(up);
        const pinned = /sticky|fixed/.test(style.position);
        const owner = up !== el && up.matches(bounded);
        // A bounded owner's own placement (a fixed popover) excuses nothing for what it holds; its scrolling does.
        const scrolls = up !== el;
        skip.x ||= (pinned && !owner) || (scrolls && /auto|scroll/.test(style.overflowX));
        skip.y ||= (pinned && !owner) || (scrolls && /auto|scroll/.test(style.overflowY));
        if (owner) {
          return { bound: up, skip, clips };
        }
        if (up !== el && /hidden|clip/.test(style.overflowX + style.overflowY)) {
          clips.push(up);
        }
        if (up.matches(".react-flow__viewport, body")) {
          return undefined;
        }
      }
      return undefined;
    },
    name(el) {
      const label = el.dataset["testid"] === undefined ? [...el.classList].join(".") : `[data-testid=${el.dataset["testid"]}]`;
      return `<${el.tagName.toLowerCase()}${label === "" ? "" : `.${label}`}> "${el.textContent.trim().replace(/\s+/g, " ").slice(0, 40)}" ${box(el)}`;
    },
  };
  Object.assign(window, { __containmentKit: kit });
}

/** Each seen element's box inside its nearest bounded ancestor's, within the tolerance (px). */
function checkBoxes(tolerance: number): string[] {
  const kit = (window as unknown as Record<string, Kit | undefined>)["__containmentKit"];
  const found: string[] = [];
  if (kit === undefined) {
    return found;
  }
  for (const el of kit.seen) {
    const owner = kit.boundOf(el);
    if (owner === undefined || el.matches("option")) {
      continue;
    }
    // What a clipping ancestor hides is not drawn: only the part inside every clip is compared (the clip's own ellipsis or lack of one is checkCuts').
    const inner = el.getBoundingClientRect().toJSON() as { left: number; right: number; top: number; bottom: number };
    for (const clip of owner.clips) {
      const at = clip.getBoundingClientRect();
      [inner.left, inner.right, inner.top, inner.bottom] = [Math.max(inner.left, at.left), Math.min(inner.right, at.right), Math.max(inner.top, at.top), Math.min(inner.bottom, at.bottom)];
    }
    const outer = owner.bound.getBoundingClientRect();
    if (inner.right <= inner.left || inner.bottom <= inner.top) {
      continue;
    }
    const out = [
      ["left", outer.left - inner.left, owner.skip.x],
      ["right", inner.right - outer.right, owner.skip.x],
      ["top", outer.top - inner.top, owner.skip.y],
      ["bottom", inner.bottom - outer.bottom, owner.skip.y],
    ] as const;
    const over = out.filter(([, by, skipped]) => !skipped && by > tolerance).map(([side, by]) => `${side} by ${String(Math.round(by * 10) / 10)}px`);
    if (over.length > 0) {
      found.push(`${kit.name(el)} sticks out of ${kit.name(owner.bound)}: ${over.join(", ")}`);
    }
  }
  return found;
}

/** Text cut without a sign: an element holding text that is wider than it, with no ellipsis, no clamp and no scroll. */
function checkCuts(): string[] {
  const kit = (window as unknown as Record<string, Kit | undefined>)["__containmentKit"];
  const found: string[] = [];
  for (const el of kit?.seen ?? []) {
    const style = getComputedStyle(el);
    const scrolls = /auto|scroll/.test(style.overflowX);
    const marked = style.textOverflow.includes("ellipsis") || (style.getPropertyValue("-webkit-line-clamp") !== "none" && style.getPropertyValue("-webkit-line-clamp") !== "");
    const inline = style.display.startsWith("inline") && !style.display.startsWith("inline-");
    const ownText = [...el.childNodes].some((node) => node.nodeType === Node.TEXT_NODE && (node.textContent ?? "").trim() !== "") || (/hidden|clip/.test(style.overflowX) && el.textContent.trim() !== "");
    // scrollWidth counts what an overhang adds, so an element holding one is left to the box check.
    const skipped = !ownText || el.querySelector("[data-overhang]") !== null || el.matches("input, textarea, select, option, canvas, img, video, [class*='react-flow']") || scrolls || inline || style.display === "contents";
    if (!skipped && !marked && el.scrollWidth > el.clientWidth + 1 && el.clientWidth > 0) {
      found.push(`${kit?.name(el) ?? ""} is cut: content ${String(el.scrollWidth)}px in ${String(el.clientWidth)}px with no ellipsis`);
    }
  }
  return found;
}

/** What sticks out of its box or is cut without a sign on `page` as it stands: one line each, naming the element, its text and its box, and the box it escaped. */
export async function containmentProblems(page: Page): Promise<string[]> {
  if (page.isClosed() || page.url() === "about:blank") {
    return [];
  }
  try {
    await page.evaluate(installKit, BOUNDED);
    const [boxes, cuts] = [await page.evaluate(checkBoxes, 0.5), await page.evaluate(checkCuts)];
    return [...boxes, ...cuts];
  } catch (error) {
    // A page that navigated away while the check ran has nothing to check.
    if (error instanceof Error && /context was destroyed|closed|Target page/.test(error.message)) {
      return [];
    }
    throw error;
  } finally {
    await page.evaluate(() => Reflect.deleteProperty(window, "__containmentKit")).catch(() => undefined);
  }
}

/** Fails, listing every problem, when anything sticks out of its box or is cut. */
export async function expectContained(page: Page, where = "the page"): Promise<void> {
  expect(await containmentProblems(page), `elements outside their boxes on ${where}`).toEqual([]);
}

/** A screenshot of `page` for a proof or a picture, taken only when it passes the invariant. */
export async function capture(page: Page, path: string, options: { fullPage?: boolean } = {}): Promise<void> {
  await expectContained(page, path);
  await page.screenshot({ path, ...options });
}
