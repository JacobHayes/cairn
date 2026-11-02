// The touch-target invariant (DESIGN, Tokens): on a touch screen every control a person taps has a
// hit area at least `--touch-target` in each direction. The hit area is read from Chrome's layout
// snapshot (e2e/snapshot.ts): the box of the control or the label it belongs to, with its own
// generated content that reaches past it, less what a clipping ancestor hides.
import type { Page } from "@playwright/test";

import { under, upFrom, type Part, type Snapshot } from "./snapshot.ts";
import { whenSettled } from "./shell.ts";

/** One control's name and hit size in a report. */
interface Offender {
  name: string;
  width: number;
  height: number;
}

const CONTROL_ROLES = new Set(["button", "menuitem", "menuitemradio", "menuitemcheckbox", "tab", "radio", "checkbox", "switch", "option", "slider"]);

/** Whether `part` is what a finger can tap: the one definition of a control. */
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

/** How a control is named in a report. */
function describe(control: Control, width: number, height: number): Offender {
  const { part } = control;
  const text = (part.attr["aria-label"] ?? [...under(part)].map((each) => each.text ?? "").join("")).trim().replace(/\s+/g, " ").slice(0, 30);
  const testId = part.attr["data-testid"];
  const parent = part.parent?.attr["class"] ?? "";
  return { name: `${part.tag}${testId === undefined ? "" : `[${testId}]`} in ${parent} "${text}"`, width, height };
}

/** A box by its edges. */
interface Edges {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

/**
 * `box` less what the ancestors from `from` up that clip it show no more of. A box placed with
 * `position: absolute` is clipped only from its containing block (the nearest placed ancestor) up,
 * and a fixed one by none, as the browser clips them. A scroller can be scrolled to the control,
 * so nothing from a scroller up counts.
 */
function clipped(box: Edges, from: Part | undefined, position: string): Edges {
  const hit = { ...box };
  if (position === "fixed") {
    return hit;
  }
  let inside = position !== "absolute";
  for (const up of upFrom(from)) {
    const clip = up.box;
    if (clip === undefined) {
      continue;
    }
    if (!inside) {
      if (clip.style.position === "static") {
        continue;
      }
      inside = true;
    }
    const [x, y] = [clip.style["overflow-x"], clip.style["overflow-y"]];
    if (/hidden|clip/.test(x)) {
      [hit.left, hit.right] = [Math.max(hit.left, clip.x), Math.min(hit.right, clip.x + clip.width)];
    }
    if (/hidden|clip/.test(y)) {
      [hit.top, hit.bottom] = [Math.max(hit.top, clip.y), Math.min(hit.bottom, clip.y + clip.height)];
    }
    if (/auto|scroll/.test(x + y)) {
      break;
    }
  }
  return hit;
}

/**
 * Where generated content placed with `position: absolute` is: Chrome gives an empty one (`content:
 * ""`, the usual way to reach a hit area past its control) no box, so it is placed from its
 * containing block (the nearest placed ancestor, from the control up) and its insets. None when
 * an inset is not a length.
 */
function placed(pseudo: Part, control: Part): Edges | undefined {
  const box = pseudo.box;
  if (box === undefined || box.style.position !== "absolute") {
    return undefined;
  }
  if (box.width > 0 && box.height > 0) {
    return { left: box.x, top: box.y, right: box.x + box.width, bottom: box.y + box.height };
  }
  const block = [...upFrom(control)].find((up) => up.box !== undefined && up.box.style.position !== "static")?.box;
  const [top, right, bottom, left] = (["top", "right", "bottom", "left"] as const).map((side) => /^-?[\d.]+px$/.test(box.style[side]) ? Number.parseFloat(box.style[side]) : Number.NaN);
  if (block === undefined || [top, right, bottom, left].some((each) => Number.isNaN(each))) {
    return undefined;
  }
  return { left: block.x + (left ?? 0), top: block.y + (top ?? 0), right: block.x + block.width - (right ?? 0), bottom: block.y + block.height - (bottom ?? 0) };
}

/** The control's hit box from the layout: its owner's box, with the boxes of its own generated content (a ::before that reaches past it), each kept to what the ancestors that clip it let through. */
function hitBox(control: Control): Edges {
  const own = control.owner.box;
  if (own === undefined) {
    return { left: 0, top: 0, right: 0, bottom: 0 };
  }
  const hit = clipped({ left: own.x, top: own.y, right: own.x + own.width, bottom: own.y + own.height }, control.owner.parent, own.style.position);
  for (const each of control.part.children.filter((child) => child.tag.startsWith("::") && child.box !== undefined && child.box.style["pointer-events"] !== "none")) {
    const at = each.box;
    const box = at === undefined ? undefined : at.style.position === "absolute" ? placed(each, control.part) : { left: at.x, top: at.y, right: at.x + at.width, bottom: at.y + at.height };
    if (at === undefined || box === undefined) {
      continue;
    }
    const extra = clipped(box, control.part, at.style.position);
    if (extra.right > extra.left && extra.bottom > extra.top) {
      [hit.left, hit.top, hit.right, hit.bottom] = [Math.min(hit.left, extra.left), Math.min(hit.top, extra.top), Math.max(hit.right, extra.right), Math.max(hit.bottom, extra.bottom)];
    }
  }
  return hit;
}

/** What the touch check found on one page: the controls under the minimum, and every measured control's hit size. */
export interface TouchReport {
  problems: string[];
  sizes: Offender[];
}

/** The page's controls against the touch-target minimum (none on a page opened with a mouse, which is not held to it). */
export async function touchReport(page: Page, snapshot: Snapshot): Promise<TouchReport> {
  const empty = { problems: [], sizes: [] };
  if (page.isClosed() || page.url() === "about:blank" || !(await whenSettled(page, () => page.evaluate(() => matchMedia("(pointer: coarse)").matches)))) {
    return empty;
  }
  const minimum = Number.parseFloat(await whenSettled(page, () => page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue("--touch-target"))));
  if (!(minimum > 0)) {
    return { problems: ["the --touch-target token is not defined 0x0"], sizes: [] };
  }
  // The tour's callout floats over the page as a popover does, also while the page under it is live.
  const popovers = snapshot.nodes.filter((part) => ("data-popover" in part.attr || part.attr["data-testid"] === "tour-callout") && part.box !== undefined);
  const [problems, sizes] = [new Array<string>(), new Array<Offender>()];
  for (const control of controlsOf(snapshot)) {
    const rect = control.owner.box;
    if (rect === undefined || rect.width === 0 || rect.height === 0) {
      continue;
    }
    // An open popover (or the tour's callout) covers part of the page beneath it: a control it overlaps is under it, not tapped, so it is measured inside the popover only.
    const covered = popovers.some((each) => each.box !== undefined && !([...upFrom(control.part)].includes(each)) && each.box.x < rect.x + rect.width && each.box.x + each.box.width > rect.x && each.box.y < rect.y + rect.height && each.box.y + each.box.height > rect.y);
    if (covered) {
      continue;
    }
    const hit = hitBox(control);
    const found = describe(control, Math.round(hit.right - hit.left), Math.round(hit.bottom - hit.top));
    sizes.push(found);
    if (found.width < minimum - 1 || found.height < minimum - 1) {
      problems.push(`${found.name} ${String(found.width)}x${String(found.height)}`);
    }
  }
  return { problems, sizes };
}
