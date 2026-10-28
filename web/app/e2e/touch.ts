// The touch-target invariant (DESIGN, Sizing): on a touch screen every control a person taps has a
// hit area at least `--touch-target` in each direction. The hit area is measured the way a finger
// meets it: the points around the control where a tap lands on it (or on the label it belongs
// to), so a padded or extended hit area counts and a neighbour that covers part of it does not.
//
// Most controls are decided from Chrome's layout snapshot (e2e/snapshot.ts): a control drawn whole
// in the window, at least the minimum each way, with nothing painted over it, meets the minimum.
// Only the rest (a small box, one something covers, one partly out of the window) are measured in
// the page, by the points around them, which is exact where a box is not.
import type { Page } from "@playwright/test";

import { under, upFrom, type Part, type Snapshot } from "./snapshot.ts";
import { whenSettled } from "./shell.ts";

/** One control's name and hit size in a report. */
interface Offender {
  name: string;
  width: number;
  height: number;
}

/** Where a control and its owner are in the page (`pathOf`), for the live probe. */
interface Place {
  control: number[];
  owner: number[];
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

/**
 * The controls `only` (each with its owner, the label a tap on it lands on, as places in the page)
 * whose hit area is under the touch-target minimum, measured by the points around them: their
 * index in `only` and hit size, named by the caller. Which elements are controls, and which are exempt, is `controlsOf`'s
 * to say over the snapshot; here only what the live page alone knows is read (where a tap lands).
 */
async function undersized(page: Page, only: Place[], minimum: number): Promise<{ at: number; width: number; height: number }[]> {
  // eslint-disable-next-line e2e/settled-reads -- the caller reads it in whenSettled
  return page.evaluate(([places, minimum]) => {
    const STEP = 2;
    const found: { at: number; width: number; height: number }[] = [];
    const find = (path: number[]): Element | undefined => path.reduce<Element | undefined>((at, index) => at?.children[index], document.documentElement);
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
    for (const [at, place] of places.entries()) {
      const [el, owner] = [find(place.control), find(place.owner)];
      if (!(el instanceof HTMLElement) || !(owner instanceof HTMLElement)) {
        throw new Error(`the control at ${place.control.join("/")} is not in the page`);
      }
      if (!el.closest(".react-flow")) {
        el.scrollIntoView({ block: "center", inline: "center" });
      }
      const rect = owner.getBoundingClientRect();
      if (rect.width === 0 || rect.height === 0 || rect.bottom < 0 || rect.top > innerHeight || rect.right < 0 || rect.left > innerWidth) {
        continue;
      }
      const [width, height] = hitSize(el, owner, rect);
      if (width < minimum - 1 || height < minimum - 1) {
        found.push({ at, width, height });
      }
    }
    return found;
  }, [only, minimum] as const);
}

const CONTROL_ROLES = new Set(["button", "menuitem", "menuitemradio", "menuitemcheckbox", "tab", "radio", "checkbox", "switch", "option", "slider"]);

/** Whether `part` is what a finger can tap. The one definition of a control: the live probe is given the controls this picks. */
function isControl(part: Part): boolean {
  const { tag, attr } = part;
  return (
    (tag === "a" && "href" in attr) ||
    ["button", "select", "textarea", "summary"].includes(tag) ||
    (tag === "input" && attr["type"]?.toLowerCase() !== "hidden") ||
    CONTROL_ROLES.has(attr["role"] ?? "") ||
    ("tabindex" in attr && attr["tabindex"] !== "-1")
  );
}

/** A control the check looks at: its place among the document's controls, its name, and the box a tap on it or its label lands in. */
interface Control {
  part: Part;
  owner: Part;
}

/** What the page's controls are held to: the controls to measure, and the ones that are exempt. */
function controlsOf(snapshot: Snapshot): Control[] {
  const found: Control[] = [];
  for (const part of snapshot.nodes) {
    if (!isControl(part)) {
      continue;
    }
    const box = part.box;
    // A link inside a run of text is exempt, as in any touch guideline; so is what is not drawn, inert, or placed by a canvas's camera (the zoom buttons make it larger).
    // A control that takes no taps (pointer-events: none) is not a target.
    if (box === undefined || (part.tag === "a" && box.style.display === "inline") || box.style.visibility === "hidden" || box.style["pointer-events"] === "none") {
      continue;
    }
    if ([...upFrom(part)].some((up) => "inert" in up.attr || up.attr["aria-hidden"] === "true" || (up.attr["class"] ?? "").split(/\s+/).includes("react-flow__viewport"))) {
      continue;
    }
    const label = part.tag === "input" ? [...upFrom(part)].find((up) => up.tag === "label") : undefined;
    found.push({ part, owner: label ?? part });
  }
  return found;
}

/** Where `part` is in the page: the index of each element from the document element down, among its parent's elements. */
function pathOf(part: Part): number[] {
  const path: number[] = [];
  for (let at = part; at.parent !== undefined && at.parent.tag !== "#document"; at = at.parent) {
    path.unshift(at.parent.children.filter((child) => !child.tag.startsWith("#") && !child.tag.startsWith("::")).indexOf(at));
  }
  return path;
}

/** How a control is named in a report. */
function describe(control: Control, width: number, height: number): Offender {
  const { part } = control;
  const text = (part.attr["aria-label"] ?? [...under(part)].map((each) => each.text ?? "").join("")).trim().replace(/\s+/g, " ").slice(0, 30);
  const testId = part.attr["data-testid"];
  const parent = part.parent?.attr["class"] ?? "";
  return { name: `${part.tag}${testId === undefined ? "" : `[${testId}]`} in ${parent} "${text}"`, width, height };
}

/** Whether the control's box says less than a tap does: it or an ancestor is clipped to a shape, or rotated or skewed, so only the points around it can say. (Rounded corners are 4px at most and are not counted.) */
function distorted(part: Part): boolean {
  return [...upFrom(part)].some((up) => {
    const style = up.box?.style;
    if (style === undefined) {
      return false;
    }
    const matrix = /^matrix\(([^)]*)\)$/.exec(style.transform);
    const [, b = "0", c = "0"] = matrix === null ? [] : (matrix[1] ?? "").split(",").map((each) => each.trim());
    const turned = style.transform !== "" && style.transform !== "none" && (matrix === null || Math.abs(Number(b)) > 1e-6 || Math.abs(Number(c)) > 1e-6);
    return (style["clip-path"] !== "" && style["clip-path"] !== "none") || turned;
  });
}

/** The control's hit box from the layout: its owner's box, with the boxes of its own generated content (a ::before that reaches past it), kept to what the ancestors that clip let through. */
function hitBox(control: Control): { left: number; top: number; right: number; bottom: number } {
  const own = control.owner.box;
  if (own === undefined) {
    return { left: 0, top: 0, right: 0, bottom: 0 };
  }
  const hit = { left: own.x, top: own.y, right: own.x + own.width, bottom: own.y + own.height };
  for (const each of control.part.children.filter((child) => child.tag.startsWith("::") && child.box !== undefined && child.box.style["pointer-events"] !== "none")) {
    const at = each.box;
    if (at !== undefined) {
      [hit.left, hit.top, hit.right, hit.bottom] = [Math.min(hit.left, at.x), Math.min(hit.top, at.y), Math.max(hit.right, at.x + at.width), Math.max(hit.bottom, at.y + at.height)];
    }
  }
  // What an ancestor that clips shows no more of takes no tap (a scroller is scrolled to the control, so it clips nothing the control is measured at).
  for (const up of upFrom(control.part.parent)) {
    const clip = up.box;
    if (clip !== undefined && /hidden|clip/.test(clip.style["overflow-x"] + clip.style["overflow-y"])) {
      [hit.left, hit.top, hit.right, hit.bottom] = [Math.max(hit.left, clip.x), Math.max(hit.top, clip.y), Math.min(hit.right, clip.x + clip.width), Math.min(hit.bottom, clip.y + clip.height)];
    }
  }
  return hit;
}

/** Whether something not part of the control is painted over `hit` or may be (Chrome gives the boxes of one paint layer one order) and takes taps. */
function coveredIn(snapshot: Snapshot, control: Control, hit: { left: number; top: number; right: number; bottom: number }): boolean {
  const paint = control.part.box?.paint ?? 0;
  const related = new Set<Part>([...upFrom(control.part), ...upFrom(control.owner), ...under(control.part), ...under(control.owner)]);
  return snapshot.nodes.some((other) => {
    const box = other.box;
    if (box === undefined || other.tag === "#text" || box.paint < paint || box.width <= 0 || box.height <= 0 || box.style["pointer-events"] === "none" || box.style.visibility !== "visible") {
      return false;
    }
    // A generated box takes taps for its element.
    if (related.has(other) || (other.tag.startsWith("::") && other.parent !== undefined && related.has(other.parent))) {
      return false;
    }
    return box.x < hit.right && box.x + box.width > hit.left && box.y < hit.bottom && box.y + box.height > hit.top;
  });
}

/** What the touch check found on one page: the controls under the minimum, and every measured control's hit size. */
export interface TouchReport {
  problems: string[];
  sizes: Offender[];
}

/** The page's controls against the touch-target minimum (none on a page opened with a mouse, which is not held to it). Measuring leaves the scroll positions as they were. */
export async function touchReport(page: Page, snapshot: Snapshot): Promise<TouchReport> {
  const empty = { problems: [], sizes: [] };
  if (page.isClosed() || page.url() === "about:blank" || !(await whenSettled(page, () => page.evaluate(() => matchMedia("(pointer: coarse)").matches)))) {
    return empty;
  }
  const minimum = Number.parseFloat(await whenSettled(page, () => page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue("--touch-target"))));
  if (!(minimum > 0)) {
    return { problems: ["the --touch-target token is not defined 0x0"], sizes: [] };
  }
  const popovers = snapshot.nodes.filter((part) => "data-popover" in part.attr && part.box !== undefined);
  const { width: windowWidth, height: windowHeight } = snapshot.viewport;
  const [sizes, doubtful] = [new Array<Offender>(), new Array<Control>()];
  for (const control of controlsOf(snapshot)) {
    const rect = control.owner.box;
    if (rect === undefined || rect.width === 0 || rect.height === 0) {
      continue;
    }
    // An open popover covers part of the page beneath it: a control it overlaps is under it, not tapped, so it is measured inside the popover only.
    const covered = popovers.some((each) => each.box !== undefined && !([...upFrom(control.part)].includes(each)) && each.box.x < rect.x + rect.width && each.box.x + each.box.width > rect.x && each.box.y < rect.y + rect.height && each.box.y + each.box.height > rect.y);
    if (covered) {
      continue;
    }
    const hit = hitBox(control);
    const [width, height] = [hit.right - hit.left, hit.bottom - hit.top];
    sizes.push(describe(control, Math.round(width), Math.round(height)));
    const whole = hit.left >= 0 && hit.top >= 0 && hit.right <= windowWidth && hit.bottom <= windowHeight;
    // A box the minimum already, drawn whole with nothing over it, is met all over. One out of the window is scrolled to and measured there, in the page, unless it is large enough that only a cover could shrink it.
    if (width >= minimum && height >= minimum && !distorted(control.part) && (!whole || !coveredIn(snapshot, control, hit))) {
      continue;
    }
    doubtful.push(control);
  }
  const places = doubtful.map((control) => ({ control: pathOf(control.part), owner: pathOf(control.owner) }));
  const rest = doubtful.length === 0 ? [] : await whenSettled(page, () => keepingScroll(page, () => undersized(page, places, minimum)));
  const problems = rest.map(({ at, width, height }) => {
    const found = describe(doubtful[at] as Control, width, height);
    return `${found.name} ${String(found.width)}x${String(found.height)}`;
  });
  return { problems, sizes };
}
