// The classic tree a row of a list or a timeline wears (DESIGN, Table): guide lines through the
// row's full height, and a 24px fold button, or a bullet on a leaf, at the row's level. The rows
// sit `position: relative` with the Table's padding at their start; the CSS is `ui/app.css`.
import type { CSSProperties } from "react";

/** How a row sits in its tree: its level, whether each ancestor has more children below (the last entry is the row's own `├` or `└`), and whether it is an open container. */
export interface TreePlace {
  depth: number;
  guides: readonly boolean[];
  open: boolean;
}

/** The guide lines through the row: each ancestor's while it has more below, the row's own elbow into its chevron or bullet, and from an open container's chevron down to its first child. */
export function TreeGuides({ depth, guides, open }: TreePlace) {
  return (
    <>
      {guides.map((more, level) =>
        more || level === depth - 1 ? <span key={level} className="tree-guide" data-more={more} data-elbow={level === depth - 1} style={{ "--level": level } as CSSProperties} aria-hidden="true" /> : null,
      )}
      {open ? <span className="tree-guide" data-down="true" style={{ "--level": depth } as CSSProperties} aria-hidden="true" /> : null}
    </>
  );
}

/** A container's fold button (`label` names what it folds), or a bullet in the same slot on a leaf. */
export function TreeFold({ container, open, label, testId, onFold }: { container: boolean; open: boolean; label: string; testId: string; onFold: () => void }) {
  return container ? (
    <button type="button" className="tree-fold" aria-expanded={open} aria-label={`${open ? "Fold" : "Open"} ${label}`} data-testid={testId} onClick={onFold}>
      <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" className="tree-chevron">
        <path d="M4 2l4 4-4 4" />
      </svg>
    </button>
  ) : (
    <span className="tree-fold tree-stub" aria-hidden="true" />
  );
}
