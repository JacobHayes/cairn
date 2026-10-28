// Rendered contrast (DESIGN, Accessibility): the words a person reads meet the contrast of the
// size they are set in, measured from what Chrome drew: each run's text colour over the
// background colours of the elements it sits in (laid over one another, with the group opacity of
// a faded row or card). Text with a gradient or an image behind it has no single background, so
// it is not decided here. The token pairs are held by the token test.
import { contrastRatio } from "../src/ui/wcag.ts";
import { upFrom, type Part, type Snapshot } from "./snapshot.ts";

/** WCAG's AA floor for text, which DESIGN holds at every size. */
export const CONTRAST_MIN = 4.5;

/** A colour as red, green, blue (0-255) and alpha. */
type Colour = [number, number, number, number];

const WHITE: Colour = [255, 255, 255, 1];

/** Reads Chrome's `rgb()` and `rgba()` strings. */
function parse(value: string): Colour | undefined {
  const found = /^rgba?\(([^)]*)\)$/.exec(value.trim());
  if (found === null) {
    return undefined;
  }
  const parts = (found[1] ?? "").split(/[\s,/]+/).filter((each) => each !== "").map(Number);
  const [red, green, blue, alpha = 1] = parts;
  return red === undefined || green === undefined || blue === undefined || parts.some(Number.isNaN) ? undefined : [red, green, blue, alpha];
}

/** `top` at `alpha` over the opaque `under`. */
function over(top: Colour, alpha: number, under: Colour): Colour {
  const a = top[3] * alpha;
  return [top[0] * a + under[0] * (1 - a), top[1] * a + under[1] * (1 - a), top[2] * a + under[2] * (1 - a), 1];
}

/** The contrast ratio of two opaque colours. */
function ratioOf(a: Colour, b: Colour): number {
  return contrastRatio([a[0] / 255, a[1] / 255, a[2] / 255], [b[0] / 255, b[1] / 255, b[2] / 255]);
}

/** The colours a run of text is drawn in and on, or nothing when no single ground lies behind it (a gradient or an image). */
function drawnOn(run: Part, fadeMin: number): { ink: Colour; ground: Colour } | undefined {
  const chain = [...upFrom(run.parent)].reverse();
  let ground = WHITE;
  let unknown = false;
  const entry = new Map<Part, Colour>();
  for (const up of chain) {
    const style = up.box?.style;
    if (style === undefined) {
      continue;
    }
    entry.set(up, ground);
    const fill = parse(style["background-color"]);
    if (fill !== undefined && fill[3] > 0) {
      ground = over(fill, 1, ground);
    }
    // An image or a gradient has no single colour, until a later opaque fill covers it.
    const image = style["background-image"] !== "none" && style["background-image"] !== "";
    unknown = image || (unknown && !(fill !== undefined && fill[3] >= 1));
  }
  const text = parse(run.box?.style.color ?? "");
  if (text === undefined || unknown) {
    return undefined;
  }
  let ink = over(text, 1, ground);
  // A faded group is blended into what is behind it as a whole. A done or out-of-focus group is faded to read as past or aside, on purpose (DESIGN, One status language), so only a fade of `fadeMin` (`--opacity-snoozed`: the lighter fade of snoozed work, which DESIGN holds above the floor) or lighter counts.
  for (const up of [...chain].reverse()) {
    const opacity = Number.parseFloat(up.box?.style.opacity ?? "1");
    const before = entry.get(up);
    if (opacity < 1 && opacity >= fadeMin && before !== undefined) {
      ground = over(ground, opacity, before);
      ink = over(ink, opacity, before);
    }
  }
  return { ink, ground };
}

/** Text a person is not expected to read at full strength: a disabled control, and what an `inert` or hidden-from-assistive-technology subtree holds. */
function excused(run: Part): boolean {
  return [...upFrom(run.parent)].some((up) => "disabled" in up.attr || up.attr["aria-disabled"] === "true" || "inert" in up.attr || up.attr["aria-hidden"] === "true");
}

/** What the rule found on `snapshot`, and the lowest ratios among the runs it could decide. */
export function contrastReport(snapshot: Snapshot, fadeMin: number): { problems: string[]; lowest: { words: string; ratio: number }[] } {
  const found = new Map<string, string>();
  const ratios: { words: string; ratio: number }[] = [];
  for (const run of snapshot.nodes) {
    const box = run.box;
    const words = (run.text ?? "").trim().replace(/\s+/g, " ");
    if (box === undefined || words === "" || box.width === 0 || box.style.visibility !== "visible" || excused(run)) {
      continue;
    }
    const drawn = drawnOn(run, fadeMin);
    if (drawn === undefined) {
      continue;
    }
    const exact = ratioOf(drawn.ink, drawn.ground);
    const ratio = Math.round(exact * 100) / 100;
    ratios.push({ words: words.slice(0, 30), ratio });
    if (exact < CONTRAST_MIN) {
      const owner = run.parent;
      const where = `${owner?.tag ?? ""}${(owner?.attr["class"] ?? "") === "" ? "" : `.${(owner?.attr["class"] ?? "").split(/\s+/).join(".")}`}`;
      const key = `${where} ${box.style.color} ${rgb(drawn.ground)}`;
      if (!found.has(key)) {
        found.set(key, `"${words.slice(0, 30)}" in <${where}> is ${ratio.toFixed(2)}:1 (${box.style.color} on ${rgb(drawn.ground)}), under ${String(CONTRAST_MIN)}:1`);
      }
    }
  }
  return { problems: [...found.values()], lowest: ratios.sort((a, b) => a.ratio - b.ratio).slice(0, 5) };
}

function rgb(colour: Colour): string {
  return `rgb(${colour.slice(0, 3).map(Math.round).join(", ")})`;
}
