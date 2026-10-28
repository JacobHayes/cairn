// DESIGN.md's contrast floors, held against design/tokens.css in both themes: a token edit that
// loses a border or a readable text colour fails here instead of in a screenshot.
import { readFileSync } from "node:fs";
import { expect, it } from "vitest";

import { contrastRatio } from "./wcag.ts";

const css = readFileSync(new URL("../../../../design/tokens.css", import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

type Theme = "light" | "dark";
/** A colour as sRGB channels (0 to 1) and alpha. */
type Rgba = [number, number, number, number];

// Every token is declared once, in the first :root block; a theme-dependent one as light-dark(light, dark).
const declared = new Map<string, string>();
for (const [, name, value] of css.slice(0, css.indexOf("\n}")).matchAll(/--([\w-]+):\s*([^;]+);/g)) {
  declared.set(name ?? "", (value ?? "").trim());
}

/** The comma-separated arguments of a function call's inside, split at the top level. */
function args(inside: string): string[] {
  const out: string[] = [];
  let depth = 0;
  let start = 0;
  for (let i = 0; i < inside.length; i += 1) {
    if (inside[i] === "(") depth += 1;
    else if (inside[i] === ")") depth -= 1;
    else if (inside[i] === "," && depth === 0) {
      out.push(inside.slice(start, i).trim());
      start = i + 1;
    }
  }
  out.push(inside.slice(start).trim());
  return out;
}

/** What `value` is in `theme`: hex, rgba(), var(), light-dark() and color-mix(in srgb, C p%, transparent). */
function evaluate(value: string, theme: Theme): Rgba {
  const hex = /^#([0-9a-f]{6})$/i.exec(value);
  if (hex) {
    const n = parseInt(hex[1] ?? "", 16);
    return [(n >> 16) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255, 1];
  }
  const call = /^([\w-]+)\((.*)\)$/.exec(value);
  const [fn = "", inside = ""] = [call?.[1], call?.[2]];
  const parts = args(inside);
  if (fn === "var") {
    const found = declared.get((parts[0] ?? "").slice(2));
    if (found === undefined) throw new Error(`unknown token ${value}`);
    return evaluate(found, theme);
  }
  if (fn === "light-dark") return evaluate(parts[theme === "light" ? 0 : 1] ?? "", theme);
  if (fn === "rgba") {
    const [r = 0, g = 0, b = 0, alpha = 1] = parts.map(Number);
    return [r / 255, g / 255, b / 255, alpha];
  }
  if (fn === "color-mix") {
    const [space, from, to] = parts;
    const share = /^(.+) (\d+(?:\.\d+)?)%$/.exec(from ?? "");
    if (space !== "in srgb" || to !== "transparent" || !share) throw new Error(`unsupported colour mix ${value}`);
    const [r, g, b, alpha] = evaluate(share[1] ?? "", theme);
    return [r, g, b, alpha * (Number(share[2]) / 100)];
  }
  throw new Error(`unsupported colour ${value}`);
}

/** The colour of `token` in `theme`, as an opaque ground: stacked from the first up, each layer over the one before. */
function over(layers: string[], theme: Theme): [number, number, number] {
  let under: [number, number, number] = [1, 1, 1];
  for (const layer of layers) {
    const [r, g, b, alpha] = evaluate(`var(--${layer})`, theme);
    under = [alpha * r + (1 - alpha) * under[0], alpha * g + (1 - alpha) * under[1], alpha * b + (1 - alpha) * under[2]];
  }
  return under;
}

/** The contrast of the opaque ink `fg` on the ground stacked from `bg`, bottom layer first. */
function ratio(theme: Theme, fg: string, bg: string[]): number {
  return contrastRatio(over([fg], theme), over(bg, theme));
}

// [foreground, background layers (bottom first), floor]
const floors: [string, string[], number][] = [
  ["color-text", ["color-surface"], 4.5],
  ["color-muted", ["color-surface"], 4.5],
  ["color-muted", ["color-ground"], 4.5],
  ["color-text", ["color-band"], 4.5],
  ["color-muted", ["color-band"], 4.5],
  ["color-error", ["color-surface"], 4.5],
  ["color-error", ["color-band"], 4.5],
  ["color-warning", ["color-surface"], 4.5],
  ["color-warning", ["color-band"], 4.5],
  ["color-success", ["color-surface"], 4.5],
  ["color-success", ["color-band"], 4.5],
  ["chrome-ink", ["chrome-bg"], 4.5],
  ["color-error", ["chrome-bg"], 4.5],
  ["chrome-muted", ["chrome-bg"], 4.5],
  ["chrome-brand", ["chrome-bg"], 4.5],
  ["color-frame", ["color-ground"], 3],
  ["color-frame", ["color-sunken"], 3],
  ["color-edge", ["color-ground"], 2],
  ["color-edge", ["color-surface"], 2],
  ["color-surface", ["color-ground"], 1.08],
];

// A marked card, row, label or line keeps its text legible on every ground a diff fill lands on, and
// the status ink set as text in a marked card keeps its own floor on paper.
for (const fill of ["color-add", "color-change", "color-del"]) {
  for (const ground of ["color-surface", "color-band", "color-ground", "color-sunken"]) {
    for (const ink of ["color-text", "color-muted"]) {
      floors.push([ink, [ground, fill], 4.5]);
    }
  }
  for (const ink of ["color-error", "color-warning", "color-success"]) {
    floors.push([ink, ["color-surface", fill], 4.5]);
  }
}

it.each<[Theme]>([["light"], ["dark"]])("%s theme keeps its contrast floors", (theme) => {
  const short = floors.filter(([fg, bg, floor]) => !(ratio(theme, fg, bg) >= floor)).map(([fg, bg, floor]) => `${fg} on ${bg.join(" under ")} needs ${String(floor)} and has ${ratio(theme, fg, bg).toFixed(2)}`);
  expect(short).toEqual([]);
});
