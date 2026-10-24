// The containment invariant (DESIGN, Nothing spills out of its box): on the page as it stands,
// no visible element sticks out of the bounded box it belongs to, and no text is visibly cut
// without an ellipsis or a clamp. Every browser test is checked after it ends (e2e/test.ts), and
// a picture is checked before it is taken (e2e/invariants.ts, `capture`), so a spec never opts in.
//
// The check reads geometry only, never words, from Chrome's layout snapshot (e2e/snapshot.ts). An
// element is checked against its nearest ancestor that is a bounded box (BOUNDED); a scroll
// container on the way excuses the axis it scrolls, and a canvas's viewport ends the walk (its
// cards are placed freely). What DESIGN lets overhang (a hanging tag, a rank tag, a popover, a
// menu, a tooltip, a connection handle) carries `data-overhang`: it is not held to the box it
// hangs from, and is itself a bounded box for what it holds.
import { classesOf, textOf, under, upFrom, type Part, type Snapshot } from "./snapshot.ts";

/** The boxes things belong to: a card, a panel, a chip or badge, a button, a list or table row, an inspector section, a toolbar, a header, and what overhangs (a popover holds its own content). */
const BOUNDED_TAGS = new Set(["button", "tr", "header"]);
const BOUNDED_CLASSES = new Set(["panel", "callout", "chip", "badge", "button", "node-row", "node-panel", "detail-section", "toast", "view-controls", "ws-head"]);

/** Whether `part` is a bounded box. */
export function isBounded(part: Part): boolean {
  return BOUNDED_TAGS.has(part.tag) || part.attr["data-testid"] === "node-card" || part.attr["role"] === "row" || "data-overhang" in part.attr || classesOf(part).some((each) => BOUNDED_CLASSES.has(each));
}

/** What the rules look at: elements Chrome drew, outside every `data-overhang` subtree's own checks. */
function seen(snapshot: Snapshot): Part[] {
  return snapshot.nodes.filter((part) => {
    const box = part.box;
    if (box === undefined || part.tag.startsWith("#") || part.tag.startsWith("::") || box.width <= 0 || box.height <= 0 || box.style.visibility !== "visible" || "data-overhang" in part.attr) {
      return false;
    }
    // What a clip-path (the visually-hidden pattern) cuts away, and what is inside an svg, is not held to a box.
    return ![...upFrom(part)].some((up) => up.tag === "svg" || (up.box !== undefined && up.box.style["clip-path"] !== "none" && up.box.style["clip-path"] !== "") || classesOf(up).includes("visually-hidden") || "hidden" in up.attr);
  });
}

/** A name for the report: the element, its text and its box. */
export function nameOf(part: Part): string {
  const box = part.box;
  const label = part.attr["data-testid"] === undefined ? classesOf(part).join(".") : `[data-testid=${part.attr["data-testid"]}]`;
  const at = box === undefined ? "" : ` [${[box.x, box.y, box.width, box.height].map((n) => String(Math.round(n * 10) / 10)).join(", ")}]`;
  return `<${part.tag}${label === "" ? "" : `.${label}`}> "${textOf(part).trim().replace(/\s+/g, " ").slice(0, 40)}"${at}`;
}

/** `part`'s nearest bounded ancestor, the axes a scroll container or a pinned (sticky or fixed) ancestor on the way excuses, and the ancestors between that clip what they hold. */
function boundOf(part: Part): { bound: Part; skip: { x: boolean; y: boolean }; clips: Part[] } | undefined {
  const skip = { x: false, y: false };
  const clips: Part[] = [];
  for (const up of upFrom(part)) {
    if (up.tag === "body") {
      return undefined;
    }
    const style = up.box?.style;
    const pinned = style !== undefined && /sticky|fixed/.test(style.position);
    const owner = up !== part && isBounded(up);
    // A bounded owner's own placement (a fixed popover) excuses nothing for what it holds; its scrolling does.
    const scrolls = up !== part;
    skip.x ||= (pinned && !owner) || (scrolls && style !== undefined && /auto|scroll/.test(style["overflow-x"]));
    skip.y ||= (pinned && !owner) || (scrolls && style !== undefined && /auto|scroll/.test(style["overflow-y"]));
    if (owner) {
      return { bound: up, skip, clips };
    }
    if (up !== part && style !== undefined && /hidden|clip/.test(style["overflow-x"] + style["overflow-y"])) {
      clips.push(up);
    }
    if (classesOf(up).includes("react-flow__viewport")) {
      return undefined;
    }
  }
  return undefined;
}

/** Each seen element's box inside its nearest bounded ancestor's, within the tolerance (px). */
function boxProblems(drawn: Part[], tolerance: number): string[] {
  const found: string[] = [];
  for (const part of drawn) {
    const owner = boundOf(part);
    const box = part.box;
    if (owner === undefined || box === undefined || part.tag === "option") {
      continue;
    }
    // What a clipping ancestor hides is not drawn: only the part inside every clip is compared (the clip's own ellipsis or lack of one is cutProblems').
    const inner = { left: box.x, right: box.x + box.width, top: box.y, bottom: box.y + box.height };
    for (const clip of owner.clips) {
      const at = clip.box;
      if (at !== undefined) {
        [inner.left, inner.right, inner.top, inner.bottom] = [Math.max(inner.left, at.x), Math.min(inner.right, at.x + at.width), Math.max(inner.top, at.y), Math.min(inner.bottom, at.y + at.height)];
      }
    }
    const outer = owner.bound.box;
    if (outer === undefined || inner.right <= inner.left || inner.bottom <= inner.top) {
      continue;
    }
    const out = [
      ["left", outer.x - inner.left, owner.skip.x],
      ["right", inner.right - (outer.x + outer.width), owner.skip.x],
      ["top", outer.y - inner.top, owner.skip.y],
      ["bottom", inner.bottom - (outer.y + outer.height), owner.skip.y],
    ] as const;
    const over = out.filter(([, by, skipped]) => !skipped && by > tolerance).map(([side, by]) => `${side} by ${String(Math.round(by * 10) / 10)}px`);
    if (over.length > 0) {
      found.push(`${nameOf(part)} sticks out of ${nameOf(owner.bound)}: ${over.join(", ")}`);
    }
  }
  return found;
}

/** The right edge of the text `part` holds itself, whatever else it holds. */
function ownTextRight(part: Part): number {
  let right = -Infinity;
  for (const child of part.children) {
    if (child.tag === "#text" && child.box !== undefined) {
      right = Math.max(right, child.box.x + child.box.width);
    }
  }
  return right;
}

/** Whether `part` is cut by a sign: an ellipsis or a clamp. */
export function isMarked(part: Part): boolean {
  const style = part.box?.style;
  const clamp = style?.["-webkit-line-clamp"] ?? "";
  return style !== undefined && (style["text-overflow"].includes("ellipsis") || (clamp !== "none" && clamp !== ""));
}

/** Text cut without a sign: an element holding text that is wider than it, with no ellipsis, no clamp and no scroll. */
function cutProblems(drawn: Part[]): string[] {
  const found: string[] = [];
  for (const part of drawn) {
    const box = part.box;
    if (box === undefined) {
      continue;
    }
    const style = box.style;
    const scrolls = /auto|scroll/.test(style["overflow-x"]);
    const inline = style.display.startsWith("inline") && !style.display.startsWith("inline-");
    const ownText = part.children.some((child) => child.tag === "#text" && (child.text ?? "").trim() !== "") || (/hidden|clip/.test(style["overflow-x"]) && textOf(part).trim() !== "");
    // scrollWidth counts what an overhang adds, so an element holding one is held to its own text's width instead.
    const holdsOverhang = [...under(part)].some((each) => "data-overhang" in each.attr);
    const skipped = !ownText || ["input", "textarea", "select", "option", "canvas", "img", "video"].includes(part.tag) || (part.attr["class"] ?? "").includes("react-flow") || scrolls || inline || style.display === "contents";
    const wide = holdsOverhang ? ownTextRight(part) > box.x + box.width + 1 : box.scroll.width > box.client.width + 1;
    if (!skipped && !isMarked(part) && wide && box.client.width > 0) {
      found.push(`${nameOf(part)} is cut: content ${String(box.scroll.width)}px in ${String(box.client.width)}px with no ellipsis`);
    }
  }
  return found;
}

/** What sticks out of its box or is cut without a sign on `snapshot`: one line each, naming the element, its text and its box, and the box it escaped. */
export function containmentProblems(snapshot: Snapshot): string[] {
  const drawn = seen(snapshot);
  return [...boxProblems(drawn, 0.5), ...cutProblems(drawn)];
}

/** What is cut by a sign (an ellipsis or a clamp) and is cut now: the near misses of the rule above. */
export function ellipsised(snapshot: Snapshot): string[] {
  return seen(snapshot)
    .filter((part) => isMarked(part) && part.box !== undefined && (part.box.scroll.width > part.box.client.width + 1 || part.box.scroll.height > part.box.client.height + 1) && part.box.client.width > 0)
    .map((part) => nameOf(part));
}
