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
    const inert = (el: Element): boolean => el.closest("[inert], [aria-hidden='true']") !== null;
    for (const el of document.querySelectorAll<HTMLElement>(selector)) {
      const style = getComputedStyle(el);
      // What is drawn on a canvas is placed and scaled by the camera, so its size is the zoom's to give (the zoom buttons make it larger); the controls around the canvas are held to the minimum.
      if (style.visibility === "hidden" || style.display === "none" || inert(el) || el.closest(".react-flow__viewport") !== null) {
        continue;
      }
      // A link inside a run of text is exempt, as in any touch guideline.
      if (el instanceof HTMLAnchorElement && style.display === "inline") {
        continue;
      }
      const label = el instanceof HTMLInputElement ? el.closest("label") : null;
      const owner = label ?? el;
      if (!el.closest(".react-flow")) {
        el.scrollIntoView({ block: "center", inline: "center" });
      }
      const rect = owner.getBoundingClientRect();
      if (rect.width === 0 || rect.height === 0 || rect.bottom < 0 || rect.top > innerHeight || rect.right < 0 || rect.left > innerWidth) {
        continue;
      }
      const [width, height] = hitSize(el, owner, rect);
      if (width < minimum - 1 || height < minimum - 1) {
        const text = (el.getAttribute("aria-label") ?? el.textContent).trim().replace(/\s+/g, " ").slice(0, 30);
        const testId = el.getAttribute("data-testid");
        found.push({ name: `${el.tagName.toLowerCase()}${testId === null ? "" : `[${testId}]`} in ${el.parentElement?.className ?? ""} "${text}"`, width, height });
      }
    }
    return found;
  }, SELECTOR);
}

/** Fails, naming each control, when any control on the page as it stands is under the touch-target minimum. */
export async function expectTouchTargets(page: Page, where: string): Promise<void> {
  const found = await whenSettled(page, () => undersized(page));
  expect.soft(
    found.map((each) => `${each.name} ${String(each.width)}x${String(each.height)}`),
    `controls under the touch target on ${where}`,
  ).toEqual([]);
}
