// F5, F6: a chain short of days, with its chain and the moves that each give it the days it
// lacks. A move is an ordinary mutation; choosing one is the caller's (`onMove`), and nothing
// is applied on its own.
import type { Schema } from "@cairn/client";

import { mutationWords, namesOf } from "../proposals/words.ts";
import { daysWords } from "../status/when.ts";
import { Button } from "../ui/kit.tsx";
import { ChainView } from "./ChainView.tsx";

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
  const names = namesOf([view.journey.graph], view.inputs.deployment.entities ?? [], view.derived.today);
  const moves = short.resolutions ?? [];
  return (
    <div className="callout callout-bad callout-short stack" data-testid="shortfall" data-days={short.shortfall_days}>
      <strong>Needs {daysWords(short.shortfall_days)} more than it has</strong>
      {moves.length === 0 ? (
        <span className="muted small">No single move resolves it; restructure the work.</span>
      ) : (
        <div className="stack">
          <span className="muted small">Each of these resolves it on its own:</span>
          {moves.map((move, at) => (
            <span key={at} className="row" data-testid="resolution" data-op={move.op}>
              {onMove === undefined ? (
                mutationWords(move, names)
              ) : (
                <Button disabled={disabled} onClick={() => { onMove(move); }}>
                  {mutationWords(move, names)}
                </Button>
              )}
            </span>
          ))}
        </div>
      )}
      <details open={onMove === undefined}>
        <summary>Why it is short</summary>
        <ChainView view={view} chain={short.chain} />
      </details>
    </div>
  );
}
