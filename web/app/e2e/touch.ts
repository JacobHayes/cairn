// The touch-target invariant (DESIGN, Sizing): on a touch screen every control a person taps has a
// hit area at least `--touch-target` in each direction. The hit area is measured the way a finger
// meets it: the points around the control where a tap lands on it (or on the label it belongs
// to), so a padded or extended hit area counts and a neighbour that covers part of it does not.
import { expect, type Page } from "@playwright/test";

import { whenSettled } from "./shell.ts";

/** What a finger can tap. */
const SELECTOR = [
  "a[href]",
  "button",
  "input:not([type=hidden])",
  "select",
  "textarea",
  "summary",
  '[role="button"]',
  '[role="menuitem"]',
  '[role="menuitemradio"]',
  '[role="menuitemcheckbox"]',
  '[role="tab"]',
  '[role="radio"]',
  '[role="checkbox"]',
  '[role="switch"]',
  '[role="option"]',
  '[role="slider"]',
  "[tabindex]:not([tabindex='-1'])",
].join(", ");

/** What the page reports for one control under the minimum. */
interface Offender {
  name: string;
  width: number;
  height: number;
}

/** `measure`, with every scroll position put back afterwards: it scrolls controls into view, and the page must stay as the test left it. */
async function keepingScroll<T>(page: Page, measure: () => Promise<T>): Promise<T> {
  const key = "__touchScroll";
   
  await page.evaluate((name) => {
    const all = [...document.querySelectorAll("*")].filter((each) => each.scrollHeight > each.clientHeight || each.scrollWidth > each.clientWidth);
    Reflect.set(window, name, all.map((each) => ({ each, top: each.scrollTop, left: each.scrollLeft })));
  }, key);
  try {
    return await measure();
  } finally {
    await page.evaluate((name) => {
      for (const { each, top, left } of Reflect.get(window, name) as { each: Element; top: number; left: number }[]) {
        each.scrollTo({ top, left, behavior: "instant" });
      }
      Reflect.deleteProperty(window, name);
    }, key);
  }
}

/** Every visible control on the page as it stands whose hit area is under the touch-target minimum, named by its tag, label and hit box. */
async function undersized(page: Page): Promise<Offender[]> {
  // eslint-disable-next-line e2e/settled-reads -- the caller reads it in whenSettled
  return page.evaluate((selector) => {
    const minimum = Number.parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--touch-target"));
    if (!(minimum > 0)) {
      return [{ name: "the --touch-target token is not defined", width: 0, height: 0 }];
    }
    const STEP = 2;
    const found: Offender[] = [];
    /** The width and height of the points around `el` where a tap lands on it or on `owner` (its label). */
    const hitSize = (el: HTMLElement, owner: HTMLElement, rect: DOMRect): [number, number] => {
      const meets = (x: number, y: number): boolean => {
        const at = document.elementFromPoint(x, y);
        return at !== null && (el.contains(at) || owner.contains(at));
      };
      const clamp = (value: number, max: number): number => Math.min(Math.max(value, 0), max - 1);
      // Not reachable by a tap at all (under a dim, a modal, another panel): not a target now.
      if (!meets(clamp(rect.left + rect.width / 2, innerWidth), clamp(rect.top + rect.height / 2, innerHeight))) {
        return [Infinity, Infinity];
      }
      // A box that is the minimum already and is met all over (a 3 by 3 sample) needs no scan of the points around it.
      if (rect.width >= minimum && rect.height >= minimum && rect.left >= 0 && rect.top >= 0 && rect.right <= innerWidth && rect.bottom <= innerHeight) {
        const across = [0.02, 0.5, 0.98];
        if (across.every((fy) => across.every((fx) => meets(clamp(rect.left + rect.width * fx, innerWidth), clamp(rect.top + rect.height * fy, innerHeight))))) {
          return [rect.width, rect.height];
        }
      }
      let [left, right, top, bottom] = [Infinity, -Infinity, Infinity, -Infinity];
      const reach = Math.ceil(minimum);
      for (let y = clamp(Math.floor(rect.top) - reach, innerHeight); y <= clamp(Math.ceil(rect.bottom) + reach, innerHeight); y += STEP) {
        for (let x = clamp(Math.floor(rect.left) - reach, innerWidth); x <= clamp(Math.ceil(rect.right) + reach, innerWidth); x += STEP) {
          if (meets(x, y)) {
            [left, right, top, bottom] = [Math.min(left, x), Math.max(right, x), Math.min(top, y), Math.max(bottom, y)];
          }
        }
      }
      return [right - left + STEP, bottom - top + STEP];
    };
    // An open popover covers part of the page beneath it: a control it overlaps is under it, not tapped, so it is measured inside the popover only.
    const popovers = [...document.querySelectorAll("[data-popover]")];
    const inert = (el: Element): boolean => el.closest("[inert], [aria-hidden='true']") !== null;
    for (const el of document.querySelectorAll<HTMLElement>(selector)) {
      const style = getComputedStyle(el);
      // A link inside a run of text is exempt, as in any touch guideline.
      if (el instanceof HTMLAnchorElement && style.display === "inline") {
        continue;
      }
      // What is drawn on a canvas is placed and scaled by the camera, so its size is the zoom's to give (the zoom buttons make it larger); the controls around the canvas are held to the minimum.
      if (style.visibility === "hidden" || style.display === "none" || inert(el) || el.closest(".react-flow__viewport") !== null) {
        continue;
      }
      const owner = (el instanceof HTMLInputElement ? el.closest("label") : null) ?? el;
      if (!el.closest(".react-flow")) {
        el.scrollIntoView({ block: "center", inline: "center" });
      }
      const rect = owner.getBoundingClientRect();
      const under = popovers.some((each) => !each.contains(el) && each.getBoundingClientRect().left < rect.right && each.getBoundingClientRect().right > rect.left && each.getBoundingClientRect().top < rect.bottom && each.getBoundingClientRect().bottom > rect.top);
      if (under || rect.width === 0 || rect.height === 0 || rect.bottom < 0 || rect.top > innerHeight || rect.right < 0 || rect.left > innerWidth) {
        continue;
      }
      const [width, height] = hitSize(el, owner, rect);
      if (width < minimum - 1 || height < minimum - 1) {
        const [text, testId] = [(el.getAttribute("aria-label") ?? el.textContent).trim().replace(/\s+/g, " ").slice(0, 30), el.getAttribute("data-testid")];
        found.push({ name: `${el.tagName.toLowerCase()}${testId === null ? "" : `[${testId}]`} in ${el.parentElement?.className ?? ""} "${text}"`, width, height });
      }
    }
    return found;
  }, SELECTOR);
}

/** Every control on the page as it stands under the touch-target minimum, one line each; none on a page opened with a mouse, which is not held to it. Measuring leaves the scroll positions as they were. */
export async function touchProblems(page: Page): Promise<string[]> {
  if (page.isClosed() || page.url() === "about:blank" || !(await whenSettled(page, () => page.evaluate(() => matchMedia("(pointer: coarse)").matches)))) {
    return [];
  }
  const found = await whenSettled(page, () => keepingScroll(page, () => undersized(page)));
  return found.map((each) => `${each.name} ${String(each.width)}x${String(each.height)}`);
}

/** Fails, naming each control, when any control on the page as it stands is under the touch-target minimum (on a touch screen only). */
export async function expectTouchTargets(page: Page, where: string): Promise<void> {
  expect(await touchProblems(page), `controls under the touch target on ${where}`).toEqual([]);
}
