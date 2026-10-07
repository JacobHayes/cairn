// C10, D5: when the acting frontier is empty, what the journey waits on, with the actions the
// diagnostic offers where they are allowed: unsnooze a snoozed node (B6), or confirm an
// `auto_reach` milestone reached before its date. Each is one patch.
import { transition, type Ready } from "../detail/model.ts";
import { Rejected } from "../detail/Rejected.tsx";
import { useNodeWrite, type NodeWrite } from "../detail/write.ts";
import { Button } from "../ui/kit.tsx";
import { DetailLink } from "./Parts.tsx";

type Cause = NonNullable<Ready["derived"]["stalled"]>["waiting_on"][number];

function CauseLine({ view, write, cause }: { view: Ready; write: NodeWrite; cause: Cause }) {
  if ("gate" in cause) {
    return (
      <li data-testid="stall-cause" data-status="gate">
        <DetailLink view={view} node={cause.gate} /> must be finished first
      </li>
    );
  }
  if ("snooze" in cause) {
    const { node, until } = cause.snooze;
    return (
      <li className="row" data-testid="stall-cause" data-status="snooze" data-node={node}>
        <span>
          <DetailLink view={view} node={node} /> is snoozed until{" "}
          {"date" in until ? until.date : <><DetailLink view={view} node={until.node} /> is finished</>}
        </span>
        <Button disabled={write.disabled} onClick={() => void write.run([{ op: "unsnooze", node }])}>
          Unsnooze
        </Button>
      </li>
    );
  }
  const { node, date } = cause.auto_reach;
  return (
    <li className="row" data-testid="stall-cause" data-status="auto_reach" data-node={node}>
      <span>
        <DetailLink view={view} node={node} /> is reached on {date}
      </span>
      <Button disabled={write.disabled} onClick={() => void write.run([transition(node, "reach")])}>
        Confirm reached now
      </Button>
    </li>
  );
}

/** D5: the stalled diagnostic, or nothing when the journey is not stalled. */
export function StalledPanel({ view }: { view: Ready }) {
  const write = useNodeWrite(view, "stalled");
  const stalled = view.derived.stalled;
  if (stalled == null) {
    return null;
  }
  return (
    <section className="callout stalled stack" role="status" data-testid="stalled">
      <strong>Nothing can be acted on right now.</strong>
      {stalled.all_blocked === true ? <span>Every remaining node is blocked.</span> : null}
      <span>The journey is waiting on:</span>
      <ul className="detail-list stack">
        {stalled.waiting_on.map((cause) => (
          <CauseLine key={JSON.stringify(cause)} view={view} write={write} cause={cause} />
        ))}
      </ul>
      <Rejected view={view} write={write} />
    </section>
  );
}
