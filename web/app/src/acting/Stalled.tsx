// C10, D5: when the acting frontier is empty, what the journey waits on, with the actions the
// diagnostic offers: unsnooze a snoozed node (B6); for a node waiting for its date (D8
// scheduled), mark a milestone done before its day, or start work before it. Each is one plain
// move: what a node waits on never stops it.
import type { ReactNode } from "react";

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
  const inkOf = (date: string) => inkClass(dateInk(date, today, undefined));
  if ("gate" in cause) {
    return (
      <li className="stall-cause" data-testid="stall-cause" data-status="gate" data-node={cause.gate}>
        <span>{title(cause.gate)} <span className="muted">· to finish first</span></span>
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
  return <ScheduledLine view={view} write={write} cause={cause.scheduled} title={title} inkOf={inkOf} />;
}

/** A node waiting for its date: a milestone is marked done, work is started; a decision is answered from its own page. */
function ScheduledLine({ view, write, cause, title, inkOf }: { view: Ready; write: NodeWrite; cause: Extract<Cause, { scheduled: unknown }>["scheduled"]; title: (node: string) => ReactNode; inkOf: (date: string) => string | undefined }) {
  const { today } = view.derived;
  const { node, date, marks_itself_done: marks } = cause;
  const kind = nodeOf(view, node)?.kind;
  const act = kind === "milestone" ? "done" : kind === "deliverable" || kind === "action" ? "start" : undefined;
  const when = kind === "milestone" ? `${marks === true ? "marks itself done " : ""}${whenWords(date, undefined, today, undefined)}` : `not before ${dateWords(date, today)}`;
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

/**
 * D5: the stalled diagnostic, or nothing when the journey is not stalled. `nodeTo` makes each
 * title lead to that address (the Summary's); where the page offers no write the causes are
 * listed without their actions. When the engine says every snooze runs past the final
 * milestone, so it is said.
 */
export function StalledPanel({ view, nodeTo }: { view: Ready; nodeTo?: (node: string) => string }) {
  const write = useNodeWrite(view, "stalled");
  const stalled = view.derived.stalled;
  if (stalled == null) {
    return null;
  }
  return (
    <section className="callout stalled stack" role="status" data-testid="stalled">
      <p className="stall-head">
        <strong>Nothing is ready yet. Waiting on:</strong>
      </p>
      {stalled.all_blocked === true ? <span>Every remaining node is blocked.</span> : null}
      {stalled.sleeps_past == null ? null : <span>Every snooze runs past the final milestone ({dateWords(stalled.sleeps_past, view.derived.today)}), so the journey cannot reach it as things stand.</span>}
      <ul className="stall-causes">
        {stalled.waiting_on.map((cause) => (
          <CauseLine key={JSON.stringify(cause)} view={view} write={write} cause={cause} reach={{ nodeTo }} />
        ))}
      </ul>
      <Rejected view={view} write={write} />
    </section>
  );
}
