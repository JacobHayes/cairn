// The window sizes the app's layout changes at are written once, as the media queries in
// frame.tsx (which the code reads through useMedia); every `@media` size in the stylesheets is
// one of them, so a stylesheet cannot change layout at a width the code does not know.
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, it } from "vitest";

import { COLUMN, PEEK_FIRST, PHONE_WIDTH, SHEET, SHORT } from "./frame.tsx";

const roots = [fileURLToPath(new URL("..", import.meta.url)), fileURLToPath(new URL("../../../../design", import.meta.url))];

function cssIn(directory: string): string[] {
  return readdirSync(directory).flatMap((name) => {
    const path = join(directory, name);
    return statSync(path).isDirectory() ? (name === "node_modules" ? [] : cssIn(path)) : name.endsWith(".css") ? [path] : [];
  });
}

/** Each `feature: size` of a media query, as written ("max-width: 719px"). */
const featuresOf = (text: string): string[] => [...text.matchAll(/\(((?:min|max)-(?:width|height)): *(\d+)px\)/g)].map(([, feature, px]) => `${feature ?? ""}: ${px ?? ""}px`);

it("every media query size in the stylesheets is one frame.tsx names", () => {
  // A boundary is also written from its other side: max-width 719px is min-width 720px.
  const known = new Set(
    [COLUMN, SHEET, PHONE_WIDTH, PEEK_FIRST, SHORT].flatMap(featuresOf).flatMap((feature) => {
      const [name = "", px = ""] = feature.split(": ");
      const [bound = "", axis = ""] = name.split("-");
      const other = bound === "max" ? `min-${axis}: ${String(parseInt(px) + 1)}px` : `max-${axis}: ${String(parseInt(px) - 1)}px`;
      return [feature, other];
    }),
  );
  const stray = roots.flatMap(cssIn).flatMap((file) => {
    const text = readFileSync(file, "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
    return [...text.matchAll(/@media([^{]*)\{/g)].flatMap(([, prelude]) => featuresOf(prelude ?? "")).filter((feature) => !known.has(feature)).map((feature) => `${file}: ${feature}`);
  });
  expect(stray).toEqual([]);
});
