// The Frontier strip (2.3): one line above the graph naming the work that can move now, best
// ranked first: `FRONTIER  #1 Kickoff · #2 Partner runs testing · +4`. Each item selects and
// centres its node on the canvas; `+4` turns the Frontier chip on, which fades the rest of the
// graph. The ranking is the engine's `next` projection over the tab's derivation, as the
// graph's rank tags are.
import { useProjected } from "../canvas/hooks.ts";
import { RANK_TAG_COUNT } from "../canvas/model.ts";
import { titleOf, type Ready } from "../detail/model.ts";
import { FRONTIER_WORDS } from "./JourneyToolbar.tsx";

export function FrontierStrip({ ready, onCentre, onAll }: { ready: Ready; onCentre: (node: string) => void; onAll: () => void }) {
  const next = useProjected(ready, { projection: "next" });
  const items = next.value?.items ?? [];
  const shown = items.slice(0, RANK_TAG_COUNT);
  if (next.value === undefined) {
    return null;
  }
  return (
    <div className="frontier-strip row" data-testid="frontier-strip">
      <span className="label" title={FRONTIER_WORDS}>
        Frontier
      </span>
      {items.length === 0 ? (
        <span className="muted" data-testid="frontier-empty">Nothing can move now</span>
      ) : (
        <>
          {shown.map((row, at) => (
            <button key={row.key} type="button" className="frontier-item" data-testid="frontier-item" data-node={row.key} onClick={() => { onCentre(row.key); }}>
              <span className="mono">#{at + 1}</span> {titleOf(ready, row.key)}
            </button>
          ))}
          {items.length > shown.length ? (
            <button type="button" className="frontier-item frontier-more" data-testid="frontier-more" onClick={onAll}>
              +{items.length - shown.length}
            </button>
          ) : null}
        </>
      )}
    </div>
  );
}
