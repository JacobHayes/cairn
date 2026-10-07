// What sits around the journey canvas: where it is drilled into, with the way back out (C4);
// the stalled surface when nothing can be acted on, naming what the journey waits on and why
// (C5, D5); and the trace bar for the open node (C7), including what the trace reaches beyond
// this canvas.
import { Link } from "react-router";

import { nodeOf, titleOf, type Ready } from "../detail/model.ts";
import type { Stalled, Trace } from "./model.ts";
import { canvasPath, type CanvasView } from "./settings.ts";

/** C4: the containers from the whole journey down to the one drilled into, each a way back. */
export function Crumbs({ ready, view }: { ready: Ready; view: CanvasView }) {
  const journey = ready.journey.header.id;
  const path: string[] = [];
  let at = view.container;
  while (at !== undefined && !path.includes(at)) {
    path.unshift(at);
    at = nodeOf(ready, at)?.parent ?? undefined;
  }
  return (
    <nav className="crumbs" aria-label="Drilled into" data-testid="crumbs">
      {view.container === undefined ? (
        <strong>Whole journey</strong>
      ) : (
        <Link to={canvasPath(journey, { ...view, container: undefined })}>Whole journey</Link>
      )}
      {path.map((key) => (
        <span key={key}>
          {" / "}
          {key === view.container ? (
            <strong data-testid="crumb-current" data-node={key}>
              {titleOf(ready, key)}
            </strong>
          ) : (
            <Link to={canvasPath(journey, { ...view, container: key })}>{titleOf(ready, key)}</Link>
          )}
        </span>
      ))}
    </nav>
  );
}

/** One thing a stalled journey waits on (D5), in words, with a link to the node. */
function Cause({ ready, view, cause }: { ready: Ready; view: CanvasView; cause: Stalled["waiting_on"][number] }) {
  const journey = ready.journey.header.id;
  const link = (key: string) => (
    <Link to={canvasPath(journey, view, key)} data-node={key}>
      {titleOf(ready, key)}
    </Link>
  );
  if ("gate" in cause) {
    return <li data-testid="stall-cause" data-status="gate">{link(cause.gate)} must be finished first</li>;
  }
  if ("snooze" in cause) {
    const until = cause.snooze.until;
    return (
      <li data-testid="stall-cause" data-status="snooze">
        {link(cause.snooze.node)} is snoozed until {"date" in until ? until.date : <>{link(until.node)} is finished</>}
      </li>
    );
  }
  return (
    <li data-testid="stall-cause" data-status="auto_reach">
      {link(cause.auto_reach.node)} is reached on {cause.auto_reach.date}
    </li>
  );
}

/** C5, D5: when the acting frontier is empty, what the journey is waiting on and why. */
export function StalledSurface({ ready, view }: { ready: Ready; view: CanvasView }) {
  const stalled = ready.derived.stalled;
  if (stalled == null) {
    return null;
  }
  return (
    <section className="callout stalled stack" role="status" data-testid="stalled">
      <strong>Nothing can be acted on right now.</strong>
      {stalled.all_blocked === true ? <span>Every remaining node is blocked.</span> : null}
      <span>The journey is waiting on:</span>
      <ul className="detail-list">
        {stalled.waiting_on.map((cause) => (
          <Cause key={JSON.stringify(cause)} ready={ready} view={view} cause={cause} />
        ))}
      </ul>
    </section>
  );
}

/** C7: trace the open node, or what its trace holds and reaches beyond this canvas. */
export function TraceBar({ ready, view, selected, trace, outside }: { ready: Ready; view: CanvasView; selected: string; trace: Trace | undefined; outside: string[] }) {
  const journey = ready.journey.header.id;
  const title = titleOf(ready, selected);
  if (!view.trace) {
    return (
      <div className="row">
        <Link to={canvasPath(journey, { ...view, trace: true }, selected)} data-testid="trace-start">
          Trace {title}
        </Link>
      </div>
    );
  }
  return (
    <div className="row" data-testid="trace-bar" data-node={selected}>
      <span>
        Tracing <strong>{title}</strong>
        {trace === undefined
          ? ""
          : `: ${String(trace.upstream.length)} upstream, ${String(trace.downstream.length)} downstream, ${String(trace.gravity_contributors.length)} of them adding to its gravity`}
        {outside.length === 0 ? "" : `; not on this canvas: ${outside.map((key) => titleOf(ready, key)).join(", ")}`}
      </span>
      <Link className="button" to={canvasPath(journey, { ...view, trace: false }, selected)} data-testid="trace-stop">
        Stop tracing
      </Link>
    </div>
  );
}
