// What a proposal's layer says about a card that its words do not: the mark's own note. Read
// aloud only; shared by the cards and the tiles that stand for them.
import type { OverlayMark } from "./overlay.ts";

export function MarkNote({ mark }: { mark: OverlayMark | undefined }) {
  return mark?.diff === undefined || mark.note === undefined ? null : <span className="visually-hidden">{mark.note}</span>;
}
