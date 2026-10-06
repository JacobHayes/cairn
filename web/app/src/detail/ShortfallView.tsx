// F5, F6: a chain short of days, with its chain and the moves that each give it the days it
// lacks. A move is an ordinary mutation; choosing one is the caller's (`onMove`), and nothing
// is applied on its own.
import type { Schema } from "@cairn/client";

import { Button } from "../ui/kit.tsx";
import { ChainView } from "./ChainView.tsx";
import { moveText, namer } from "./explain.ts";
import type { Mutation, Ready } from "./model.ts";

export function ShortfallView({
  view,
  short,
  onMove,
  disabled = false,
}: {
  view: Ready;
  short: Schema<"ShortChain">;
  onMove?: ((move: Mutation) => void) | undefined;
  disabled?: boolean;
}) {
  const name = namer(view);
  const moves = short.resolutions ?? [];
  return (
    <div className="callout callout-bad stack" data-testid="shortfall" data-days={short.shortfall_days}>
      <strong>{short.shortfall_days} days short</strong>
      <ChainView view={view} chain={short.chain} />
      {moves.length === 0 ? (
        <span className="muted">No single move resolves it; restructure the work.</span>
      ) : (
        <div className="stack">
          <span className="muted">Each of these resolves it on its own:</span>
          {moves.map((move, at) => (
            <span key={at} className="row" data-testid="resolution" data-op={move.op}>
              <span>{moveText(move, name)}</span>
              {onMove === undefined ? null : (
                <Button disabled={disabled} onClick={() => { onMove(move); }}>
                  Use this
                </Button>
              )}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}
