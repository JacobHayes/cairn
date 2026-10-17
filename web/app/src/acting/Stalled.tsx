// C10, D5: when the acting frontier is empty, what the journey waits on, with the actions the
// diagnostic offers where they are allowed: unsnooze a snoozed node (B6), or confirm an
// `auto_reach` milestone reached before its date. Each is one patch.
import { Link } from "react-router";

import { titleOf, transition, type Ready } from "../detail/model.ts";
import { Rejected } from "../detail/Rejected.tsx";
import { useNodeWrite, type NodeWrite } from "../detail/write.ts";
import { dateWords } from "../timeline/model.ts";
import { Button } from "../ui/kit.tsx";
import { DetailLink } from "./Parts.tsx";

/** Where a node's title leads when it is not the screen's own detail link, and whether the causes' actions are offered. */
interface Reach {
  nodeTo?: ((node: string) => string) | undefined;
  readOnly: boolean;
}

function NodeTitle({ view, node, reach }: { view: Ready; node: string; reach: Reach }) {
  return reach.nodeTo === undefined ? <DetailLink view={view} node={node} /> : <Link to={reach.nodeTo(node)} data-node={node}>{titleOf(view, node)}</Link>;
}

type Cause = NonNullable<Ready["derived"]["stalled"]>["waiting_on"][number];

function CauseLine({ view, write, cause, reach }: { view: Ready; write: NodeWrite; cause: Cause; reach: Reach }) {
  if ("gate" in cause) {
    return (
      <li data-testid="stall-cause" data-status="gate">
        <NodeTitle view={view} node={cause.gate} reach={reach} /> must be finished first
      </li>
    );
  }
  if ("snooze" in cause) {
    const { node, until } = cause.snooze;
    return (
      <li className="row" data-testid="stall-cause" data-status="snooze" data-node={node}>
        <span>
          <NodeTitle view={view} node={node} reach={reach} /> is snoozed until{" "}
          {"date" in until ? dateWords(until.date, view.derived.today) : <><NodeTitle view={view} node={until.node} reach={reach} /> is finished</>}
        </span>
        {reach.readOnly ? null : (
          <Button disabled={write.disabled} onClick={() => void write.run([{ op: "unsnooze", node }])}>
            Unsnooze
          </Button>
        )}
      </li>
    );
  }
  const { node, date } = cause.auto_reach;
  return (
    <li className="row" data-testid="stall-cause" data-status="auto_reach" data-node={node}>
      <span>
        <NodeTitle view={view} node={node} reach={reach} /> is reached on {date}
      </span>
      {reach.readOnly ? null : (
        <Button disabled={write.disabled} onClick={() => void write.run([transition(node, "reach")])}>
          Confirm reached now
        </Button>
      )}
    </li>
  );
}

/** Whether every cause is a snooze to a date after `endDate`: the journey sleeps past its end. */
function pastEnd(causes: Cause[], endDate: string | undefined): boolean {
  return endDate !== undefined && causes.length > 0 && causes.every((cause) => "snooze" in cause && "date" in cause.snooze.until && cause.snooze.until.date > endDate);
}

/**
 * D5: the stalled diagnostic, or nothing when the journey is not stalled. `nodeTo` makes each
 * title lead to that address (the Summary's), and `readOnly` (an archived journey, A19) lists
 * the causes without their actions. With `endDate` (the final milestone's), snoozes that run past
 * it are said to.
 */
export function StalledPanel({ view, nodeTo, readOnly = false, endDate }: { view: Ready; nodeTo?: (node: string) => string; readOnly?: boolean; endDate?: string | undefined }) {
  const write = useNodeWrite(view, "stalled");
  const stalled = view.derived.stalled;
  if (stalled == null) {
    return null;
  }
  return (
    <section className="callout stalled stack" role="status" data-testid="stalled">
      <strong>Nothing can be acted on right now.</strong>
      {stalled.all_blocked === true ? <span>Every remaining node is blocked.</span> : null}
      {pastEnd(stalled.waiting_on, endDate) ? <span>Every snooze runs past the final milestone ({dateWords(endDate ?? "", view.derived.today)}), so the journey cannot reach it as things stand.</span> : null}
      <span>The journey is waiting on:</span>
      <ul className="detail-list stack">
        {stalled.waiting_on.map((cause) => (
          <CauseLine key={JSON.stringify(cause)} view={view} write={write} cause={cause} reach={{ nodeTo, readOnly }} />
        ))}
      </ul>
      <Rejected view={view} write={write} />
    </section>
  );
}
