// What a proposal's layer says about a card that its words do not: that it needs a choice, and the
// mark's own note. Read aloud only; shared by the cards and the tiles that stand for them.
import type { OverlayMark } from "./overlay.ts";

export function MarkNote({ mark }: { mark: OverlayMark | undefined }) {
  return mark?.diff === undefined || (mark.choice !== true && mark.note === undefined) ? null : (
    <span className="visually-hidden">
      {mark.choice === true ? "Needs a choice. " : ""}
      {mark.note}
    </span>
  );
}
