// What sits around the journey canvas: the stalled surface when nothing can be acted on,
// naming what the journey waits on and why (C5, D5). The trace bar and the ladder are the
// canvas's own (Chrome.tsx).
import { Link } from "react-router";

import { titleOf, type Ready } from "../detail/model.ts";
import type { Stalled } from "./model.ts";
import { canvasPath, type CanvasView } from "./settings.ts";

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
