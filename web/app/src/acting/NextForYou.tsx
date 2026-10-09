// C10, C16: the line above the list when the viewer holds work here but none of it can be done
// yet: what is theirs next, and what it waits on.
import type { Ready } from "../detail/model.ts";
import type { MineEntry } from "../journeys/mine.ts";
import { nextForYou, nextForYouWords } from "./yours.ts";

export function NextForYou({ view, entries, nothing = true }: { view: Ready; entries: readonly MineEntry[]; nothing?: boolean }) {
  const next = nextForYou(view, entries);
  if (next === undefined) {
    return null;
  }
  return (
    <p className="next-for-you" role="status" data-testid="next-for-you" data-node={next.node}>
      {nothing ? <strong>Nothing needs you now. </strong> : null}
      {nextForYouWords(view, next)}
    </p>
  );
}
