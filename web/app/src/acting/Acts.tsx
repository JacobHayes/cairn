// C11's per-kind actions on one node, inline on a triage card and on a next list row (C10),
// each one patch through the shell's write path, its rejection shown beside it (A15; D4 with
// its bypass). Answering and snoozing are node detail's own editors (5.1), so a draft started
// here is the one the panel shows. A placeholder breaks down into pieces that open a proposal
// (B10). An unassigned node offers to assign its owner (D2); a card offers pass, which writes
// nothing (C11). Undecided work may be finished, and says beside its acts that it may not
// apply (D4).
import { useState } from "react";
import { Link } from "react-router";

import { canvasPath, DEFAULT_VIEW } from "../canvas/settings.ts";
import { newAttachmentKey } from "../detail/Attachments.tsx";
import { MayNotApply, SkipForm } from "../detail/Actions.tsx";
import { AnswerEditor } from "../detail/AnswerEditor.tsx";
import { SnoozeEditor } from "../detail/editors.tsx";
import { nodeDetail, transition, type Ready } from "../detail/model.ts";
import { Rejected } from "../detail/Rejected.tsx";
import { entityName } from "../detail/sections.tsx";
import { useFormDraft, useNodeWrite, type NodeWrite } from "../detail/write.ts";
import { Button, Field } from "../ui/kit.tsx";
import { BreakDown } from "../proposals/Entries.tsx";
import { actsFor, assignOwner, doneMutations, hasArtifact, type Act, type Facts } from "./acts.ts";

/** D4: the acts that finish a node, beside which undecided work says it may not apply. */
const FINISHING: Act[] = ["done", "reach", "answer"];

/** G2, C11: done for a deliverable that requires an artifact and has none: its link and the completion, one patch. */
function DoneWithArtifact({ write, node }: { write: NodeWrite; node: string }) {
  const form = useFormDraft<string>(write.journey, node, "done-artifact");
  if (form.draft === undefined) {
    return (
      <Button primary disabled={write.disabled} onClick={() => { form.open("", write.seen); }}>
        Done
      </Button>
    );
  }
  const drafted = form.draft;
  const { value } = drafted;
  const send = async () => {
    if (await write.run(doneMutations(node, { key: newAttachmentKey(), url: value.trim() }), drafted)) {
      form.close();
    }
  };
  return (
    <span className="row" data-testid="done-artifact">
      <Field aria-label="Artifact address" placeholder="Its artifact's address" value={value} onChange={(event) => { form.change(event.target.value); }} />
      <Button primary disabled={write.disabled || value.trim() === ""} onClick={() => void send()}>
        Link and mark done
      </Button>
      <Button onClick={() => { form.close(); write.dismiss(); }}>Cancel</Button>
    </span>
  );
}

/** D2: assign an unassigned node's owner inline. */
export function AssignOwner({ view, write, node }: { view: Ready; write: NodeWrite; node: string }) {
  const [entity, setEntity] = useState("");
  return (
    <span className="row" data-testid="assign">
      <select className="select" aria-label="Assign owner" value={entity} onChange={(event) => { setEntity(event.target.value); }}>
        <option value="">Assign owner</option>
        {(view.inputs.deployment.entities ?? []).map((each) => (
          <option key={each.key} value={each.key}>
            {entityName(view, each.key)}
          </option>
        ))}
      </select>
      <Button disabled={write.disabled || entity === ""} onClick={() => void write.run([assignOwner(node, entity)])}>
        Assign
      </Button>
    </span>
  );
}

/** One of the kind's buttons; answer and snooze are editors of their own, drawn below. */
function ActButton({ view, write, facts, act, onSkip }: { view: Ready; write: NodeWrite; facts: Facts; act: Act; onSkip: () => void }) {
  const key = facts.node.key;
  const run = (mutations: Parameters<NodeWrite["run"]>[0]) => () => void write.run(mutations);
  switch (act) {
    case "start":
      return <Button disabled={write.disabled} onClick={run([transition(key, "start")])}>Start</Button>;
    case "done":
      return facts.node.requires_artifact === true && !hasArtifact(view, key) ? (
        <DoneWithArtifact write={write} node={key} />
      ) : (
        <Button primary disabled={write.disabled} onClick={run(doneMutations(key))}>Done</Button>
      );
    case "reach":
      return <Button primary disabled={write.disabled} onClick={run([transition(key, "reach")])}>Mark reached</Button>;
    case "skip":
      return <Button disabled={write.disabled} onClick={onSkip}>Skip</Button>;
    case "breakdown":
      return <BreakDown ready={view} node={facts.node} />;
    case "atomic":
      return <Button primary disabled={write.disabled} onClick={run([{ op: "set_atomic", node: key, atomic: true }])}>Mark atomic</Button>;
    case "canvas":
      return <Link className="button" to={canvasPath(view.journey.header.id, DEFAULT_VIEW, key)}>Open on canvas</Link>;
    case "answer":
    case "snooze":
      return null;
  }
}

/** C11: node `facts`' actions for its kind, assign owner when unassigned, and pass when `onPass` is given. */
export function Acts({ view, facts, onPass }: { view: Ready; facts: Facts; onPass?: (() => void) | undefined }) {
  const write = useNodeWrite(view, `acts:${facts.node.key}`);
  const skip = useFormDraft<string>(write.journey, facts.node.key, "skip");
  const detail = nodeDetail(view, facts.node.key);
  const acts = actsFor(facts);
  if (detail === undefined) {
    return null;
  }
  return (
    <div className="stack acts" data-testid="acts" data-node={facts.node.key} data-acts={acts.join(" ")}>
      <div className="row">
        {acts.map((act) => (
          <ActButton key={act} view={view} write={write} facts={facts} act={act} onSkip={() => { skip.open("", write.seen); }} />
        ))}
        {onPass === undefined ? null : (
          <Button onClick={onPass} data-testid="pass" title="Pass (P): later in this pass; nothing is saved">
            Pass
          </Button>
        )}
      </div>
      {acts.some((act) => FINISHING.includes(act)) ? <MayNotApply view={view} node={facts.node.key} /> : null}
      {skip.draft === undefined ? null : <SkipForm write={write} node={facts.node.key} form={skip} />}
      {acts.includes("answer") ? <AnswerEditor view={view} detail={detail} /> : null}
      {facts.derived.unassigned === true ? <AssignOwner view={view} write={write} node={facts.node.key} /> : null}
      {acts.includes("snooze") ? <SnoozeEditor view={view} detail={detail} /> : null}
      <Rejected view={view} write={write} />
    </div>
  );
}
