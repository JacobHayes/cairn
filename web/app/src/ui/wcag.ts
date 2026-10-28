// WCAG's contrast arithmetic, shared by the token test (which checks the token pairs) and the
// structural checks (which check what a page drew): one definition of luminance and of the ratio.

/** An opaque colour as red, green, blue, each 0 to 1. */
export type Rgb = readonly [number, number, number];

/** The relative luminance of `rgb`. */
export function luminance(rgb: Rgb): number {
  const [r = 0, g = 0, b = 0] = rgb.map((channel) => (channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** The contrast ratio of two opaque colours: 1 to 21, unrounded. */
export function contrastRatio(a: Rgb, b: Rgb): number {
  const [high = 0, low = 0] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (high + 0.05) / (low + 0.05);
}
