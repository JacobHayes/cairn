// Per-kind actions (D1): the transitions the node's state machine offers from where it is,
// a skip with the reason D1 requires, and for a decision its answer (B2), each one patch
// whose rejection shows here, with a bypass when a guard failed (D4). A decision that fills a
// role or pins a milestone says so: those values are edited by answering it (E3).
import { Button, Field } from "../ui/kit.tsx";
import { AnswerEditor } from "./AnswerEditor.tsx";
import { isBlocked, movesFrom, transition, type Move, type NodeDetail, type Ready } from "./model.ts";
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

function SkipForm({ write, node, form }: { write: NodeWrite; node: string; form: ReturnType<typeof useFormDraft<string>> }) {
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

export function Actions({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const write = useNodeWrite(view, `actions:${detail.node.key}`);
  const { node, record } = detail;
  const skip = useFormDraft<string>(write.journey, node.key, "skip");
  const moves = movesFrom(node.kind, record.state);
  const startedEarly = record.state === "active" && isBlocked(detail.derived, record.state);
  return (
    <div className="stack" data-testid="actions">
      <div className="row">
        {moves.map((move) =>
          move === "skip" ? (
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
        {startedEarly ? <span className="muted">Started early: still blocked.</span> : null}
      </div>
      {skip.draft === undefined ? null : <SkipForm write={write} node={node.key} form={skip} />}
      <Rejected view={view} write={write} />
      {node.kind === "decision" ? <AnswerEditor view={view} detail={detail} /> : null}
    </div>
  );
}
