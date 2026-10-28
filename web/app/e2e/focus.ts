// The focus ring (DESIGN, Accessibility): every interactive element shows a visible 2px ring when
// the keyboard reaches it, and nothing covers it. One Tab walk over a screen visits each stop in
// the page's order and reads, in the page, the ring the focused control (or the box that draws it
// for the control: a field's wrapper, a label) has and whether anything is over it. Focus and
// hover state are not in the layout snapshot, so this is the one rule read live; the walk moves
// focus and opens tips, so the caller reaches the state again afterwards.
import type { Page } from "@playwright/test";

/** The ring DESIGN states, in px. */
const RING_PX = 2;

/** More stops than any screen has: a walk that does not come back to its first stop ends here, and fails. */
const STOPS_MAX = 400;

/** What the page says of the control focus is on now. */
interface Stop {
  name: string;
  ring: boolean;
}

/** Moves to the next stop's facts in the page: the focused control, and the box that draws its ring (kept in `window.__ring` for `ringTrouble`). "end" when focus left the page or came round to a stop already seen. */
function reachStop(ringPx: number): Stop | "end" {
  const el = document.activeElement;
  const seen = Reflect.get(window, "__stops") as Set<Element>;
  if (el === null || el === document.body || el === document.documentElement || seen.has(el)) {
    return "end";
  }
  seen.add(el);
  /** The ring `box` draws: an outline, or a hard shadow, of `ringPx` or more, and how far out from its border its middle lies (negative: inside). */
  const draws = (box: Element): { reach: number } | undefined => {
    // A line on a canvas is an svg group: its stroke is its ring.
    if (box instanceof SVGGElement) {
      const stroke = box.querySelector(".react-flow__edge-path");
      return stroke !== null && getComputedStyle(stroke).stroke !== "none" && Number.parseFloat(getComputedStyle(stroke).strokeWidth) >= ringPx ? { reach: 0 } : undefined;
    }
    const style = getComputedStyle(box);
    if (style.outlineStyle !== "none" && Number.parseFloat(style.outlineWidth) >= ringPx && !/rgba\([^)]*, 0\)$/.test(style.outlineColor)) {
      return { reach: Number.parseFloat(style.outlineOffset) + Number.parseFloat(style.outlineWidth) / 2 };
    }
    for (const each of style.boxShadow.split(/,(?![^(]*\))/)) {
      const lengths = [...each.matchAll(/(-?[\d.]+)px/g)].map((found) => Number.parseFloat(found[1] ?? "0"));
      if (lengths.length >= 4 && (lengths[2] ?? 0) === 0 && (lengths[3] ?? 0) >= ringPx) {
        return { reach: (each.includes("inset") ? -1 : 1) * ((lengths[3] ?? 0) / 2) };
      }
    }
    return undefined;
  };
  // The control itself, or the box around it that draws the ring for it (a field's wrapper, a label).
  let ringed: { box: Element; reach: number } | undefined;
  for (let up: Element | null = el, depth = 0; up !== null && depth < 4 && ringed === undefined; up = up.parentElement, depth += 1) {
    const ring = draws(up);
    ringed = ring === undefined ? undefined : { box: up, reach: ring.reach };
  }
  Reflect.set(window, "__ring", ringed);
  const label = (el.getAttribute("aria-label") ?? el.textContent).trim().replace(/\s+/g, " ").slice(0, 30);
  const testId = el.getAttribute("data-testid");
  const classes = typeof el.className === "string" && el.className !== "" ? `.${el.className.split(/\s+/).join(".")}` : "";
  return { name: `${el.tagName.toLowerCase()}${testId === null ? "" : `[${testId}]`}${classes} "${label}"`, ring: ringed !== undefined };
}

/** What is wrong with the ring `reachStop` found, in the page: it is cut by a box that clips it (a side whose edge shows but whose ring lies outside the clip), or something is on top of it (the element on top where the middle of each side of the box lies a few pixels in, when it is not part of the control; a neighbour's border may overlap the edge itself). A side whose edge is itself outside a box that scrolls or clips it is cut off with the control, not the ring's fault. */
function ringTrouble(): string | undefined {
  const ring = Reflect.get(window, "__ring") as { box: Element; reach: number } | undefined;
  // A line's stroke is its own ring, and its box says nothing of where it is drawn.
  if (ring === undefined || ring.box instanceof SVGElement) {
    return undefined;
  }
  const { box: ringed, reach } = ring;
  const rect = ringed.getBoundingClientRect();
  const inset = Math.min(3, rect.width / 4, rect.height / 4);
  const [midX, midY] = [rect.left + rect.width / 2, rect.top + rect.height / 2];
  // Each side: the point on its edge, the point in its ring, and the point a few pixels in.
  const sides: { name: string; edge: [number, number]; ring: [number, number]; inner: [number, number] }[] = [
    { name: "top", edge: [midX, rect.top], ring: [midX, rect.top - reach], inner: [midX, rect.top + inset] },
    { name: "bottom", edge: [midX, rect.bottom], ring: [midX, rect.bottom + reach], inner: [midX, rect.bottom - inset] },
    { name: "left", edge: [rect.left, midY], ring: [rect.left - reach, midY], inner: [rect.left + inset, midY] },
    { name: "right", edge: [rect.right, midY], ring: [rect.right + reach, midY], inner: [rect.right - inset, midY] },
  ];
  const clips: { box: Element; rect: DOMRect; scrolls: boolean }[] = [];
  for (let up = ringed.parentElement; up !== null; up = up.parentElement) {
    const style = getComputedStyle(up);
    if (/hidden|auto|scroll|clip/.test(style.overflowX + style.overflowY)) {
      clips.push({ box: up, rect: up.getBoundingClientRect(), scrolls: /auto|scroll/.test(style.overflowX + style.overflowY) });
    }
  }
  /** The nearest box that cuts `point` off. */
  const cut = ([x, y]: [number, number]) => clips.find(({ rect }) => x < rect.left || x > rect.right || y < rect.top || y > rect.bottom);
  for (const side of sides) {
    if (cut(side.edge) !== undefined) {
      continue;
    }
    // Focus scrolls a control to its scroller's edge, and a ring a pixel past it is the browser's doing, as it is at the window's.
    // The camera of a canvas places its cards, at its pane's edge too.
    const clipper = ringed.closest(".react-flow__viewport") === null ? cut(side.ring) : undefined;
    if (clipper !== undefined && !clipper.scrolls) {
      return `cut on its ${side.name} side by <${clipper.box.tagName.toLowerCase()}.${typeof clipper.box.className === "string" ? clipper.box.className.split(/\s+/).join(".") : ""}>`;
    }
    const hit = document.elementFromPoint(...side.inner);
    // A popover or tip the stop before opened is still closing (a peek lets go after a beat, so the pointer can cross into it): it covers nothing the person is on.
    if (hit !== null && !ringed.contains(hit) && !hit.contains(ringed) && hit.closest('[data-popover], [role="tooltip"]') === null) {
      return `covered by <${hit.tagName.toLowerCase()}.${typeof hit.className === "string" ? hit.className.split(/\s+/).join(".") : ""}>`;
    }
  }
  return undefined;
}

/** Walks the Tab order from the top of the page and returns one line for every stop that shows no ring or has it covered. */
export async function focusRingProblems(page: Page): Promise<string[]> {
   
  await page.evaluate(() => {
    // Blurring leaves Chrome's starting point for Tab where focus was: a focusable stand-in at the top of the page puts it there.
    const start = document.createElement("div");
    start.tabIndex = -1;
    document.body.prepend(start);
    start.focus();
    Reflect.set(window, "__stops", new Set<Element>([start]));
  });
  const problems: string[] = [];
  for (let stop = 0; stop < STOPS_MAX; stop += 1) {
    await page.keyboard.press("Tab");
    // eslint-disable-next-line e2e/settled-reads -- the key press is the settling
    const now = await page.evaluate(reachStop, RING_PX);
    if (now === "end") {
      return problems;
    }
    // eslint-disable-next-line e2e/settled-reads -- as above
    const trouble = await page.evaluate(ringTrouble);
    if (!now.ring) {
      problems.push(`${now.name} shows no ${String(RING_PX)}px focus ring`);
    } else if (trouble !== undefined) {
      problems.push(`${now.name} has its focus ring ${trouble}`);
    }
  }
  return [...problems, `the Tab walk did not come back to its start in ${String(STOPS_MAX)} stops`];
}
