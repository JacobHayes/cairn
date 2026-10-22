// C10, D5: when the acting frontier is empty, what the journey waits on, with the actions the
// diagnostic offers where they are allowed: unsnooze a snoozed node (B6); for a node waiting for
// its date (D8 scheduled), mark a milestone done before its day, or start work early (finishing
// it early needs a reason, so it is not offered here). Each is one patch.

import { nodeOf, transition, type Ready } from "../detail/model.ts";
import { ACT_LABELS } from "../detail/offers.ts";
import { Rejected } from "../detail/Rejected.tsx";
import { useNodeWrite, type NodeWrite } from "../detail/write.ts";
import { SNOOZE_NODE_TAIL, snoozeEndWords } from "../status/snooze.ts";
import { dateInk, dateWords, inkClass, whenWords } from "../status/when.ts";
import { Button } from "../ui/kit.tsx";
import { NodeLink } from "../detail/parts.tsx";

/** Where a node's title leads when it is not the screen's own detail link. */
interface Reach {
  nodeTo?: ((node: string) => string) | undefined;
}

type Cause = NonNullable<Ready["derived"]["stalled"]>["waiting_on"][number];

/** One cause as a row: the node's title, then what it waits on in plain words (a date the way the app says dates, not a verb that suits only some titles), then its action at the row's end. */
function CauseLine({ view, write, cause, reach }: { view: Ready; write: NodeWrite; cause: Cause; reach: Reach }) {
  const { today } = view.derived;
  const title = (node: string) => <NodeLink view={view} node={node} {...(reach.nodeTo === undefined ? {} : { to: reach.nodeTo(node) })} />;
  const inkOf = (date: string) => inkClass(dateInk(date, today));
  if ("gate" in cause) {
    return (
      <li className="stall-cause" data-testid="stall-cause" data-status="gate" data-node={cause.gate}>
        <span>{title(cause.gate)} <span className="muted">· must be finished first</span></span>
      </li>
    );
  }
  if ("snooze" in cause) {
    const { node, until } = cause.snooze;
    return (
      <li className="stall-cause" data-testid="stall-cause" data-status="snooze" data-node={node}>
        <span>
          <span className="stall-snoozed">{title(node)}</span>{" "}
          <span className="muted">
            · snoozed until {"date" in until ? <span className={inkOf(until.date)}>{snoozeEndWords(view, until)}</span> : <>{title(until.node)}{SNOOZE_NODE_TAIL}</>}
          </span>
        </span>
        {!write.writable ? null : (
          <Button disabled={write.disabled} onClick={() => void write.run([{ op: "unsnooze", node }])}>
            {ACT_LABELS.unsnooze}
          </Button>
        )}
      </li>
    );
  }
  const { node, date, marks_itself_done: marks } = cause.scheduled;
  const kind = nodeOf(view, node)?.kind;
  // A milestone is marked done whenever it happens; work may start before its date; a decision waits.
  const act = kind === "milestone" ? "done" : kind === "deliverable" || kind === "action" ? "start" : undefined;
  const when = kind === "milestone" ? `${marks === true ? "marks itself done " : ""}${whenWords(date, undefined, today)}` : `not before ${dateWords(date, today)}`;
  return (
    <li className="stall-cause" data-testid="stall-cause" data-status="scheduled" data-node={node}>
      <span>{title(node)} <span className={inkOf(date) ?? "muted"}>· {when}</span></span>
      {!write.writable || act === undefined ? null : (
        <Button disabled={write.disabled} onClick={() => void write.run([transition(node, act === "done" ? "complete" : "start")])}>
          {ACT_LABELS[act]}
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
 * title lead to that address (the Summary's); where the page offers no write the causes are
 * listed without their actions. With `endDate` (the final milestone's), snoozes that run past
 * it are said to.
 */
export function StalledPanel({ view, nodeTo, endDate }: { view: Ready; nodeTo?: (node: string) => string; endDate?: string | undefined }) {
  const write = useNodeWrite(view, "stalled");
  const stalled = view.derived.stalled;
  if (stalled == null) {
    return null;
  }
  return (
    <section className="callout stalled stack" role="status" data-testid="stalled">
      <p className="stall-head">
        <strong>Waiting on these before anything can move:</strong>
      </p>
      {stalled.all_blocked === true ? <span>Every remaining node is blocked.</span> : null}
      {pastEnd(stalled.waiting_on, endDate) ? <span>Every snooze runs past the final milestone ({dateWords(endDate ?? "", view.derived.today)}), so the journey cannot reach it as things stand.</span> : null}
      <ul className="stall-causes">
        {stalled.waiting_on.map((cause) => (
          <CauseLine key={JSON.stringify(cause)} view={view} write={write} cause={cause} reach={{ nodeTo }} />
        ))}
      </ul>
      <Rejected view={view} write={write} />
    </section>
  );
}
