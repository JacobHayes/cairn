// The window sizes the app's layout changes at are written once, as the media queries in
// frame.tsx (which the code reads through useMedia); every `@media` size in the stylesheets is
// one of them, so a stylesheet cannot change layout at a width the code does not know.
import { expect, it } from "vitest";

import { featuresOf, mediaFeatures } from "../../e2e/stylesheets.ts";
import { COLUMN, PEEK_FIRST, PHONE_WIDTH, SHEET, SHORT } from "./frame.tsx";

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
  const stray = mediaFeatures().filter(({ feature }) => !known.has(feature)).map(({ file, feature }) => `${file}: ${feature}`);
  expect(stray).toEqual([]);
});
