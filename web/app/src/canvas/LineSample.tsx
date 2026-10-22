// A kind of line drawn as the canvas draws it (C1): solid with an arrowhead for a requirement, dotted
// with a hollow diamond for a condition, a bar for a stage opening; a date relationship is dashed,
// ending in an open arrowhead. The Key and a line's card show the same glyph, drawn from look.ts.
import { DATED, DOTTED, MARKERS } from "./look.ts";
import type { LineKind } from "./model.ts";

/** A date relationship as the canvas draws it for the selected node: a dashed line ending in an open arrowhead. */
export function DateSample() {
  return (
    <svg className="line-sample line-sample-dates" width="44" height="14" viewBox="0 0 44 14" aria-hidden="true">
      <path d="M2 7 H40" stroke="currentColor" fill="none" strokeDasharray={DATED} />
      <path d="M34 2.500 L41 7 L34 11.500" stroke="currentColor" fill="none" />
    </svg>
  );
}

export function LineSample({ kind }: { kind: LineKind }) {
  const marker = MARKERS[kind];
  const dash = kind === "requires" ? undefined : DOTTED;
  return (
    <svg className="line-sample" width="44" height="14" viewBox="0 0 44 14" aria-hidden="true">
      <path d="M2 7 H32" stroke="currentColor" fill="none" strokeDasharray={dash} />
      {marker === "arrow" ? <path d="M42 7 L33 2.500 L33 11.500 Z" fill="currentColor" /> : null}
      {marker === "diamond" ? <path d="M42 7 L37.500 2.500 L33 7 L37.500 11.500 Z" fill="var(--color-surface)" stroke="currentColor" /> : null}
      {marker === "bar" ? <path d="M40 2 V12" stroke="currentColor" strokeWidth="2" /> : null}
    </svg>
  );
}
