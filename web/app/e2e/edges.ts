// The page against its window (DESIGN, Layout): from 720px wide the document never scrolls
// down, and nothing is drawn past the window's side edges. Plain functions over Chrome's layout
// snapshot (e2e/snapshot.ts); a pane that scrolls or clips inside the window is excused for what it holds,
// but is itself held to the window, and the frame that clips at the window's edge excuses nothing.
import { nameOf, seen } from "./containment.ts";
import { classesOf, upFrom, type Part, type Snapshot } from "./snapshot.ts";

/** The width from which the page is a desktop's, one that never scrolls the document (DESIGN, Layout). */
export const DESKTOP_MIN = 720;

/** The document scrolling down at a desktop width: its content is taller than its window. */
export function pageHeightProblems(snapshot: Snapshot): string[] {
  const box = snapshot.nodes.find((part) => part.tag === "html" && part.box !== undefined)?.box;
  if (box === undefined || snapshot.viewport.width < DESKTOP_MIN) {
    return [];
  }
  return box.scroll.height > box.client.height ? [`the page scrolls down: ${String(box.scroll.height)}px of content in a ${String(box.client.height)}px window, at ${String(snapshot.viewport.width)}px wide`] : [];
}

/** Whether `part` is held in at the window's `side` by a box that scrolls or clips it inside the window (a table's scroller, a pane), or is placed by a canvas's camera. A box that reaches the window's edge holds nothing in: what it clips is what is cut off. */
function heldIn(part: Part, side: "left" | "right", width: number): boolean {
  return [...upFrom(part.parent)].some((up) => {
    const box = up.box;
    if (classesOf(up).includes("react-flow__viewport")) {
      return true;
    }
    return box !== undefined && /auto|scroll|hidden|clip/.test(box.style["overflow-x"]) && (side === "left" ? box.x > 0.5 : box.x + box.width < width - 0.5);
  });
}

/** What is drawn past the window's left or right edge, other than inside a region that scrolls or clips it. */
export function edgeProblems(snapshot: Snapshot): string[] {
  const { width } = snapshot.viewport;
  return seen(snapshot).flatMap((part) => {
    const box = part.box;
    if (box === undefined || part.tag === "html" || part.tag === "body" || classesOf(part).includes("react-flow__viewport")) {
      return [];
    }
    const [left, right] = [box.x, box.x + box.width];
    return (left < -0.5 && !heldIn(part, "left", width)) || (right > width + 0.5 && !heldIn(part, "right", width)) ? [`${nameOf(part)} is drawn past the window's edge (${String(Math.round(left))} to ${String(Math.round(right))} of ${String(width)})`] : [];
  });
}
