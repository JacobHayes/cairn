// C10, C11, E1: what a row offers at its right end: its primary action (Done, or Done... when
// a note or link is needed first; Start for work not begun; Mark reached; Decide, which opens
// the inspector's form), and who owns it (You, or an inline Assign when nobody does). Each is
// one patch through the shell's write path, its rejection shown under the row (A15, D4).
// Anything more is one click away in the inspector. The write's feedback is the shell's
// notices; this is the one place a row sends from.
import type { ReactNode } from "react";
import { useNavigate } from "react-router";

import { useViewer } from "../data/react.ts";
import { DoneButton, EvidenceForm } from "../detail/DoneEvidence.tsx";
import { transition, type Ready } from "../detail/model.ts";
import { Rejected } from "../detail/Rejected.tsx";
import { entityName, resolveEntity } from "../detail/sections.tsx";
import { useFormDraft, useNodeWrite, type NodeWrite } from "../detail/write.ts";
import { Button } from "../ui/kit.tsx";
import { actsFor, assignOwner, doneMutations, factsOf, missingEvidence, type EvidenceDraft } from "./acts.ts";
import type { NodeRow } from "./why.ts";

/** C11: "Decide" opens the inspector, opens the node's answer form, and puts the focus in it once it is drawn. */
function focusAnswerForm(node: string): void {
  let frames = 0;
  let opened = false;
  const look = () => {
    const detail = document.querySelector(`[data-testid="node-detail"][data-node="${node}"]`);
    const field = detail?.querySelector<HTMLElement>('[data-testid="answer-editor"] :is(select, input, textarea)');
    const answer = detail?.querySelector<HTMLElement>('[data-testid="actions"] button.primary');
    if (field !== null && field !== undefined) {
      field.focus();
      return;
    }
    if (answer !== null && answer !== undefined && !opened) {
      opened = true;
      answer.click();
    }
    if (frames++ < 30) {
      requestAnimationFrame(look);
    }
  };
  requestAnimationFrame(look);
}

/** Whether one of the viewer's entities, through any merge, owns the row. */
function ownedBy(view: Ready, row: NodeRow, entities: readonly string[]): boolean {
  const mine = new Set(entities.map((key) => resolveEntity(view, key)));
  return (row.owners ?? []).some((key) => mine.has(resolveEntity(view, key)));
}

/**
 * E1: pick an owner for an unowned row, in the owner's slot: choosing sends it. The slot says
 * "Unassigned" quietly until the row is hovered, focused or selected, which swaps in the picker.
 */
function AssignInline({ view, write, node }: { view: Ready; write: NodeWrite; node: string }) {
  return (
    <span className="next-assign-slot">
      <span className="next-owner next-unassigned">Unassigned</span>
      <select className="next-assign next-owner-other" aria-label="Assign owner" data-testid="assign" disabled={write.disabled} value="" onChange={(event) => { void write.run([assignOwner(node, event.target.value)]); }}>
        <option value="">Assign</option>
        {(view.inputs.deployment.entities ?? []).map((each) => (
          <option key={each.key} value={each.key}>
            {entityName(view, each.key)}
          </option>
        ))}
      </select>
    </span>
  );
}

/**
 * The row's owner slot and primary action, and under the row the form and rejection they open.
 * With `owner` off (Mine, where every row is the viewer's) the slot says nothing unless the row
 * is unowned.
 */
export function RowActions({ view, row, to, owner = true }: { view: Ready; row: NodeRow; to: string; owner?: boolean }): ReactNode {
  const navigate = useNavigate();
  const { viewer } = useViewer();
  const write = useNodeWrite(view, `acts:${row.key}`, row.key);
  const evidence = useFormDraft<EvidenceDraft | string>(write.journey, row.key, "done-artifact");
  const facts = factsOf(view, row.key);
  if (facts === undefined) {
    return null;
  }
  const acts = actsFor(facts);
  const needs = missingEvidence(view, facts.node);
  const run = (mutations: Parameters<NodeWrite["run"]>[0]) => () => void write.run(mutations);
  const owners = (row.owners ?? []).map((key) => entityName(view, key)).join(", ");
  return (
    <>
      <div className="next-row-side">
        {row.unassigned === true ? (
          <AssignInline view={view} write={write} node={row.key} />
        ) : !owner ? null : ownedBy(view, row, viewer?.entities ?? []) ? (
          <span className="next-owner">You</span>
        ) : (
          <span className="next-owner next-owner-other" title={owners === "" ? undefined : `Owned by ${owners}`}>
            {owners}
          </span>
        )}
        <span className="next-row-act">
          {acts.includes("start") ? <Button disabled={write.disabled} onClick={run([transition(row.key, "start")])}>Start</Button> : null}
          {acts.includes("done") ? (
            needs.artifact || needs.note ? (
              <DoneButton write={write} form={evidence} />
            ) : (
              <Button primary disabled={write.disabled} onClick={run(doneMutations(row.key))}>Done</Button>
            )
          ) : null}
          {acts.includes("reach") ? <Button primary disabled={write.disabled} onClick={run([transition(row.key, "reach")])}>Mark reached</Button> : null}
          {acts.includes("answer") ? (
            <Button
              primary
              onClick={() => {
                void navigate(to);
                focusAnswerForm(row.key);
              }}
            >
              Decide
            </Button>
          ) : null}
          {acts.includes("atomic") ? <Button disabled={write.disabled} onClick={run([{ op: "set_atomic", node: row.key, atomic: true }])}>Mark atomic</Button> : null}
        </span>
      </div>
      <EvidenceForm write={write} node={row.key} needs={needs} form={evidence} />
      <Rejected view={view} write={write} />
    </>
  );
}
