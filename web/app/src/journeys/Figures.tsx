// What the journey index shows of each journey beyond its name (C16, 4.8): its progress
// (done over in scope), its next milestone with its date, one flag (`STALLED`, `N OVERDUE` or
// `N STALE`, plus `+N`), and how many of its nodes are the viewer's to act on now. The index
// holds only summaries, so each journey is derived in the tab, as "mine" already does, and
// the engine's status summary read from it.
import { useJourney } from "../data/react.ts";
import type { Ready } from "../detail/model.ts";
import { flagOf, progressOf, useStatusSummary } from "./figures-model.ts";
import { useMineOf } from "./mine.ts";

function ReadyFigures({ ready }: { ready: Ready }) {
  const model = useStatusSummary(ready);
  const mine = useMineOf(ready.journey.header.id);
  const frontier = new Set(ready.derived.acting_frontier);
  const actionable = mine.status === "ready" ? mine.entries.filter((entry) => frontier.has(entry.node)).length : 0;
  const progress = model === undefined ? undefined : progressOf(model);
  const flag = flagOf(ready, model);
  const next = model?.upcoming[0];
  return (
    <>
      <td className="mono" data-testid="row-progress" data-done={progress?.done ?? ""} data-in-scope={progress?.inScope ?? ""}>
        {progress === undefined ? "" : `${String(progress.done)}/${String(progress.inScope)}`}
      </td>
      <td data-testid="row-milestone">{next === undefined ? <span className="muted small">none</span> : `${next.title} ${next.date}`}</td>
      <td className="mono" data-testid="row-flag">{flag}</td>
      <td className="mono" data-testid="row-mine">{actionable === 0 ? "" : `${String(actionable)} ready`}</td>
    </>
  );
}

/** The four cells after the status, for journey `id`; empty while it is derived. */
export function Figures({ id }: { id: string }) {
  const journey = useJourney(id);
  return journey.status === "ready" ? (
    <ReadyFigures ready={journey} />
  ) : (
    <>
      <td />
      <td />
      <td />
      <td />
    </>
  );
}
