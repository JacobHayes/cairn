// C10, C11, E1: what a row offers at its right end: the actions the node offers everywhere
// (detail/offers.ts: Mark done, or Mark done... when a note or link is needed first; Start; Mark
// reached; Decide, Break down and Mark atomic, which open or finish in the inspector), and who owns it (You, or an inline Assign when nobody does). Each is
// one patch through the shell's write path, its rejection shown under the row (A15, D4).
// Anything more is one click away in the inspector. The write's feedback is the shell's
// notices; this is the one place a row sends from.
import type { ReactNode } from "react";
import { useNavigate } from "react-router";

import { useViewer } from "../data/react.ts";
import { useReviewing } from "../proposals/layer.ts";
import { ActButton, type Opens } from "../detail/ActButton.tsx";
import { EvidenceForm, type Needs } from "../detail/DoneEvidence.tsx";
import type { Ready } from "../detail/model.ts";
import { offered } from "../detail/offers.ts";
import { OwnerSelect } from "../detail/parts.tsx";
import { Rejected } from "../detail/Rejected.tsx";
import { ownerWords, UNASSIGNED } from "../detail/sections.tsx";
import { useFormDraft, useNodeWrite, type NodeWrite } from "../detail/write.ts";
import { doneMutations, evidenceDraft, factsOf, missingEvidence, type EvidenceDraft } from "./acts.ts";
import type { NodeRow } from "./why.ts";

/**
 * E1: pick an owner for an unowned row, in the owner's slot: choosing sends it. The picker is
 * always there (a touch or keyboard user never has to find it); on paper the slot says "Unassigned".
 */
function AssignInline({ view, write, node }: { view: Ready; write: NodeWrite; node: string }) {
  return (
    <span className="row-assign-slot">
      <span className="row-owner row-unassigned">{UNASSIGNED}</span>
      <OwnerSelect view={view} write={write} node={node} placeholder="Assign" className="row-assign" testId="assign" />
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
 * is unowned. With `column` the screen has an Owner column that already says who (the inspector
 * assigns), so the row says nothing of the owner.
 */
export function RowActions({ view, row, to, owner = true, column = false, yours = false }: { view: Ready; row: NodeRow; to: string; owner?: boolean; column?: boolean; yours?: boolean }): ReactNode {
  const navigate = useNavigate();
  const { viewer } = useViewer();
  const write = useNodeWrite(view, `acts:${row.key}`, row.key);
  const evidence = useFormDraft<EvidenceDraft | string>(write.journey, row.key, "done-artifact");
  const facts = factsOf(view, row.key);
  const reviewing = useReviewing();
  // Under a proposal's layer the rows are a picture of the future: no action, no assignment.
  if (facts === undefined || reviewing !== undefined) {
    return null;
  }
  const offers = offered(view, row.key);
  const acts = [...(offers.primary === undefined ? [] : [offers.primary]), ...offers.secondary];
  const needs = missingEvidence(view, facts.node);
  // C11: the inspector opens the form or the answer for a choice that arrives with the navigation (detail/Actions.tsx).
  const open = (what: Opens) => {
    if (what === "evidence") {
      if (evidence.draft === undefined) {
        evidence.open({ artifact: "", note: "" }, write.seen);
      }
    } else {
      void navigate(to, { state: { choice: { node: row.key, id: what } } });
    }
  };
  const owners = ownerWords(view, row.owners, viewer?.identities);
  return (
    <>
      <div className="row-side">
        {column ? null : row.unassigned === true ? (
          <AssignInline view={view} write={write} node={row.key} />
        ) : !owner ? null : (
          <span className={yours ? "row-owner" : "row-owner row-owner-other"} title={`Owned by ${owners}`}>
            {owners}
          </span>
        )}
        <span className="row-act">
          {acts.map((act) => (
            <ActButton key={act} act={act} view={view} node={row.key} write={write} waiting={offers.waiting} open={open} />
          ))}
        </span>
      </div>
      <RowEvidence write={write} node={row.key} needs={needs} form={evidence} />
      <Rejected view={view} write={write} />
    </>
  );
}
