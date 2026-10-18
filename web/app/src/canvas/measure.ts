// The width of a card's title text as the browser sets it (C15): cards.ts sizes every card from
// these widths and NodeCard draws the title in the same face, so what layout counted is what is
// drawn. The width is measured with a canvas on the loaded face (the body font at the title's
// weight), in ems so one measure serves every size a title is set at. A tab installs it once
// before its first layout (boot.ts); the unit tests install a fixed model (measure.test-support.ts),
// since Node has no fonts to measure.

/** A text's width in ems of the title face. */
export type EmWidth = (text: string) => number;

let measure: EmWidth | undefined;

/** Installs the measure every size in cards.ts reads. */
export function setEmWidth(next: EmWidth): void {
  measure = next;
}

/** `text`'s width in ems of the title face. */
export function emWidthOf(text: string): number {
  if (measure === undefined) {
    throw new Error("no text measure installed: boot installs the browser's, a test installs its own");
  }
  return measure(text);
}

/** The title weight (DESIGN, Typography: a card's title is 600). */
const TITLE_WEIGHT = 600;
/** The body tracking, in ems (DESIGN, Typography). */
const TRACKING_EM = 0.01;
/** The size the face is measured at (px): large, so rounding is a small share of the width. */
const SAMPLE_PX = 100;

/** Loads the title face, then measures with it: awaited before the first layout, so no title is counted in a fallback face. */
export async function installTitleMeasure(): Promise<void> {
  const family = getComputedStyle(document.documentElement).getPropertyValue("--font-body").trim();
  const font = `${String(TITLE_WEIGHT)} ${String(SAMPLE_PX)}px ${family}`;
  // A face that cannot be fetched leaves the browser's fallback, which is then what is measured, as it is what is drawn.
  await document.fonts.load(font, "Aa").catch(() => []);
  await document.fonts.ready;
  const context = document.createElement("canvas").getContext("2d");
  if (context === null) {
    throw new Error("no 2d canvas to measure text with");
  }
  context.font = font;
  // The body text's tracking (--type-letter-spacing, 0.01em) is part of every width the browser lays out.
  context.letterSpacing = `${String(SAMPLE_PX * TRACKING_EM)}px`;
  const widths = new Map<string, number>();
  setEmWidth((text) => {
    const known = widths.get(text);
    if (known !== undefined) {
      return known;
    }
    const width = context.measureText(text).width / SAMPLE_PX;
    widths.set(text, width);
    return width;
  });
}
