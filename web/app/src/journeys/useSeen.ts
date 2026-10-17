// Whether an element is near enough to the person's view to be worth work: in the visible part of
// its scroller or within one screen of it. A long table asks this of each row, so only the rows
// near the view derive their journeys (and release them when they leave), not every row at once.
import { useEffect, useState } from "react";

/** The nearest ancestor that scrolls, or none when the page itself does. */
function scrollerOf(element: Element): Element | null {
  for (let parent = element.parentElement; parent !== null; parent = parent.parentElement) {
    const { overflowY } = getComputedStyle(parent);
    if (overflowY === "auto" || overflowY === "scroll") {
      return parent;
    }
  }
  return null;
}

/**
 * Whether `element` is within a screen of the view, kept current as the person scrolls. Always
 * true when `always` is (a row that must stay counted), and where the browser cannot tell.
 */
export function useSeen(element: Element | null, always: boolean): boolean {
  const [seen, setSeen] = useState(always);
  useEffect(() => {
    if (always || element === null || typeof IntersectionObserver === "undefined") {
      return;
    }
    const root = scrollerOf(element);
    const margin = `${String(root?.clientHeight ?? globalThis.innerHeight)}px 0px`;
    const watch = new IntersectionObserver((entries) => { setSeen(entries.at(-1)?.isIntersecting ?? false); }, { root, rootMargin: margin });
    watch.observe(element);
    return () => { watch.disconnect(); };
  }, [element, always]);
  return always || typeof IntersectionObserver === "undefined" || seen;
}
