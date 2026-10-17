// C10, C11, E1: what a row offers at its right end: its primary action (Done, or Done... when
// a note or link is needed first; Start for work not begun; Mark reached; Decide, which opens
// the inspector's form), and who owns it (You, or an inline Assign when nobody does). Each is
// one patch through the shell's write path, its rejection shown under the row (A15, D4).
// Anything more is one click away in the inspector. The write's feedback is the shell's
// notices; this is the one place a row sends from.
import type { ReactNode } from "react";
import { useNavigate } from "react-router";

import { useViewer } from "../data/react.ts";
import { DoneButton, EvidenceForm, type Needs } from "../detail/DoneEvidence.tsx";
import { transition, type Ready } from "../detail/model.ts";
import { Rejected } from "../detail/Rejected.tsx";
import { entityName } from "../detail/sections.tsx";
import { useFormDraft, useNodeWrite, type NodeWrite } from "../detail/write.ts";
import { Button } from "../ui/kit.tsx";
import { actsFor, assignOwner, doneMutations, evidenceDraft, factsOf, missingEvidence, type EvidenceDraft } from "./acts.ts";
import type { NodeRow } from "./why.ts";
import { ownedBy } from "./yours.ts";

/**
 * E1: pick an owner for an unowned row, in the owner's slot: choosing sends it. The slot says
 * "Unassigned" quietly until the row is hovered, focused or selected, which swaps in the picker.
 */
function AssignInline({ view, write, node }: { view: Ready; write: NodeWrite; node: string }) {
  return (
    <span className="row-assign-slot">
      <span className="row-owner row-unassigned">Unassigned</span>
      <select className="row-assign row-owner-other" aria-label="Assign owner" data-testid="assign" disabled={write.disabled} value="" onChange={(event) => { void write.run([assignOwner(node, event.target.value)]); }}>
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

/** The fields Done... asks for in the row, kept as the row's own draft until the write lands. */
function RowEvidence({ write, node, needs, form }: { write: NodeWrite; node: string; needs: Needs; form: ReturnType<typeof useFormDraft<EvidenceDraft | string>> }) {
  const drafted = form.draft;
  if (drafted === undefined) {
    return null;
  }
  return (
    <EvidenceForm
      needs={needs}
      value={evidenceDraft(drafted.value)}
      sending={write.pending ? "saving" : false}
      disabled={write.disabled}
      onChange={form.change}
      onSend={(added) => {
        void write.run(doneMutations(node, added), drafted).then((landed) => {
          if (landed) {
            form.close();
          }
        });
      }}
      onClose={() => {
        form.close();
        write.dismiss();
      }}
    />
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
      <div className="row-side">
        {row.unassigned === true ? (
          <AssignInline view={view} write={write} node={row.key} />
        ) : !owner ? null : ownedBy(view, row.owners, viewer?.entities ?? []) ? (
          <span className="row-owner">You</span>
        ) : (
          <span className="row-owner row-owner-other" title={owners === "" ? undefined : `Owned by ${owners}`}>
            {owners}
          </span>
        )}
        <span className="row-act">
          {acts.includes("start") ? <Button disabled={write.disabled} onClick={run([transition(row.key, "start")])}>Start</Button> : null}
          {acts.includes("done") ? (
            needs.artifact || needs.note ? (
              <DoneButton
                disabled={write.disabled}
                onOpen={() => {
                  if (evidence.draft === undefined) {
                    evidence.open({ artifact: "", note: "" }, write.seen);
                  }
                }}
              />
            ) : (
              <Button primary disabled={write.disabled} onClick={run(doneMutations(row.key))}>Done</Button>
            )
          ) : null}
          {acts.includes("reach") ? <Button primary disabled={write.disabled} onClick={run([transition(row.key, "reach")])}>Mark reached</Button> : null}
          {acts.includes("answer") ? (
            <Button
              primary
              // C11: the node's detail opens its answer form and focuses it when it arrives with this choice (detail/Actions.tsx).
              onClick={() => void navigate(to, { state: { choice: { node: row.key, id: "decide" } } })}
            >
              Decide
            </Button>
          ) : null}
          {acts.includes("atomic") ? <Button disabled={write.disabled} onClick={run([{ op: "set_atomic", node: row.key, atomic: true }])}>Mark atomic</Button> : null}
        </span>
      </div>
      <RowEvidence write={write} node={row.key} needs={needs} form={evidence} />
      <Rejected view={view} write={write} />
    </>
  );
}
