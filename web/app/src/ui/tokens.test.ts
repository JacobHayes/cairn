// DESIGN.md's contrast floors, held against design/tokens.css in both themes: a token edit that
// loses a border or a readable text colour fails here instead of in a screenshot.
import { readFileSync } from "node:fs";
import { expect, it } from "vitest";

const css = readFileSync(new URL("../../../../design/tokens.css", import.meta.url), "utf8");

/** Every colour token declared in `text`: a hex, or a `var()` of another token (resolved by the caller). */
function declared(text: string): Map<string, string> {
  const found = new Map<string, string>();
  for (const [, name, value] of text.matchAll(/--([\w-]+):\s*(#[0-9a-f]{6}|var\(--[\w-]+\))(?=;)/gi)) {
    found.set(name ?? "", value ?? "");
  }
  return found;
}

/** `tokens` with each `var(--x)` replaced by what x resolves to. */
function resolved(tokens: Map<string, string>): Map<string, string> {
  const out = new Map<string, string>();
  for (const [name, value] of tokens) {
    let hex = value;
    for (let hop = 0; hop < 4 && !hex.startsWith("#"); hop += 1) {
      hex = tokens.get(hex.slice(6, -1)) ?? "";
    }
    out.set(name, hex);
  }
  return out;
}

// Light is the base block; the data-theme dark block (the last one) overrides it.
const raw = declared(css.slice(0, css.indexOf("@media (prefers-color-scheme: dark)")));
const light = resolved(raw);
const dark = resolved(new Map([...raw, ...declared(css.slice(css.indexOf(':root[data-theme="dark"]')))]));

function luminance(hex: string): number {
  const [r = 0, g = 0, b = 0] = [1, 3, 5].map((i) => {
    const c = parseInt(hex.slice(i, i + 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function ratio(tokens: Map<string, string>, a: string, b: string): number {
  const [hi = 0, lo = 0] = [luminance(tokens.get(a) ?? "#000000"), luminance(tokens.get(b) ?? "#000000")].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

// [foreground, background, floor]
const floors: [string, string, number][] = [
  ["color-text", "color-surface", 4.5],
  ["color-muted", "color-surface", 4.5],
  ["color-muted", "color-ground", 4.5],
  ["color-text", "color-band", 4.5],
  ["color-muted", "color-band", 4.5],
  ["color-error", "color-surface", 4.5],
  ["color-error", "color-band", 4.5],
  ["color-warning", "color-surface", 4.5],
  ["color-warning", "color-band", 4.5],
  ["color-success", "color-surface", 4.5],
  ["chrome-ink", "chrome-bg", 4.5],
  ["chrome-muted", "chrome-bg", 4.5],
  ["chrome-brand", "chrome-bg", 4.5],
  ["color-frame", "color-ground", 3],
  ["color-frame", "color-sunken", 3],
  ["color-edge", "color-ground", 2],
  ["color-edge", "color-surface", 2],
  ["color-surface", "color-ground", 1.08],
];

it.each([
  ["light", light],
  ["dark", dark],
])("%s theme keeps its contrast floors", (_theme, tokens) => {
  const short = floors.filter(([fg, bg, floor]) => !(ratio(tokens, fg, bg) >= floor)).map(([fg, bg, floor]) => `${fg} on ${bg} needs ${String(floor)}`);
  expect(short).toEqual([]);
});
