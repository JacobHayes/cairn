// Per-kind actions (D1): the transitions the node's state machine offers from where it is,
// a skip with the reason D1 requires, and for a decision its answer (B2), each one patch
// whose rejection shows here, with a bypass when a guard failed (D4). Finishing undecided
// work is accepted, and says beside the action that it may not apply (D4). A decision that fills a
// role or pins a milestone says so: those values are edited by answering it (E3).
import { missingEvidence, type EvidenceDraft } from "../acting/acts.ts";
import { mayNotApply } from "../data/activity.ts";
import { Button, Field } from "../ui/kit.tsx";
import { AnswerEditor } from "./AnswerEditor.tsx";
import { DoneButton, EvidenceForm } from "./DoneEvidence.tsx";
import { movesFrom, startedEarly, titleOf, transition, unansweredOf, type Move, type NodeDetail, type Ready } from "./model.ts";
import { Rejected } from "./Rejected.tsx";
import { useFormDraft, useNodeWrite, type NodeWrite } from "./write.ts";

const LABEL: Record<Move, string> = {
  start: "Start",
  stop: "Stop",
  complete: "Complete",
  reach: "Mark reached",
  skip: "Skip",
  reopen: "Reopen",
};

/**
 * D4: beside a finishing action on undecided work, that finishing it is accepted but may not
 * apply, naming the decisions it waits on; nothing when the node is not undecided.
 */
export function MayNotApply({ view, node }: { view: Ready; node: string }) {
  const unanswered = unansweredOf(view, node);
  if (unanswered.length === 0) {
    return null;
  }
  return (
    <span className="muted small" data-testid="may-not-apply" data-unanswered={unanswered.join(" ")}>
      {mayNotApply(unanswered, (key) => titleOf(view, key))}.
    </span>
  );
}

export function SkipForm({ write, node, form }: { write: NodeWrite; node: string; form: ReturnType<typeof useFormDraft<string>> }) {
  const reason = form.draft?.value ?? "";
  const onDone = form.close;
  return (
    <span className="row" data-testid="skip-form">
      <Field aria-label="Why skip it" placeholder="Why" value={reason} onChange={(event) => { form.change(event.target.value); }} />
      <Button
        primary
        disabled={write.disabled || reason.trim() === ""}
        onClick={() => {
          void write.run([transition(node, "skip", reason)], form.draft).then((landed) => {
            if (landed) {
              onDone();
            }
          });
        }}
      >
        Skip
      </Button>
      <Button onClick={onDone}>Cancel</Button>
    </span>
  );
}

/** B6: a transition on a snoozed container clears its snooze, and with it the hold on everything beneath. */
function LiftsSnooze({ view, node }: { view: Ready; node: string }) {
  const held = Object.values(view.derived.nodes).filter((derived) => derived.snoozed_via === node).length;
  if (held === 0 || view.journey.graph.state?.snoozes?.[node] === undefined) {
    return null;
  }
  return (
    <span className="muted small" data-testid="lifts-snooze">
      Moving this on lifts its snooze, and the hold on the {held} open {held === 1 ? "item" : "items"} beneath it.
    </span>
  );
}

export function Actions({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const write = useNodeWrite(view, `actions:${detail.node.key}`, detail.node.key);
  const { node, record } = detail;
  const skip = useFormDraft<string>(write.journey, node.key, "skip");
  const evidence = useFormDraft<EvidenceDraft | string>(write.journey, node.key, "done-evidence");
  // G2, G4: work that requires a note or a link and has none is finished by Done..., which adds them with the completion.
  const needs = missingEvidence(view, node);
  const evidenceFirst = needs.artifact || needs.note;
  // C11, B10: a placeholder cannot be finished until it is broken down or marked atomic.
  const moves = movesFrom(node.kind, record.state).filter((move) => !(move === "complete" && detail.derived.needs_breakdown === true));
  const early = startedEarly(detail.derived, record.state);
  const finishes = moves.includes("complete") || moves.includes("reach") || (node.kind === "decision" && record.state === "open");
  return (
    <div className="stack" data-testid="actions">
      <div className="row">
        {moves.map((move) =>
          move === "complete" && evidenceFirst ? (
            <DoneButton key={move} write={write} form={evidence} />
          ) : move === "skip" ? (
            <Button key={move} disabled={write.disabled} onClick={() => { skip.open("", write.seen); }}>
              {LABEL[move]}
            </Button>
          ) : (
            <Button
              key={move}
              primary={move === "complete" || move === "reach"}
              disabled={write.disabled}
              onClick={() => void write.run([transition(node.key, move)])}
            >
              {LABEL[move]}
            </Button>
          ),
        )}
        {early ? <span className="muted small">Started early: still blocked.</span> : null}
      </div>
      {moves.length > 0 ? <LiftsSnooze view={view} node={node.key} /> : null}
      {finishes ? <MayNotApply view={view} node={node.key} /> : null}
      {skip.draft === undefined ? null : <SkipForm write={write} node={node.key} form={skip} />}
      <EvidenceForm write={write} node={node.key} needs={needs} form={evidence} />
      <Rejected view={view} write={write} />
      {node.kind === "decision" ? <AnswerEditor view={view} detail={detail} /> : null}
    </div>
  );
}
