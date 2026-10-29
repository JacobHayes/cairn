// Where proposals start, on earlier screens. A journey's `⋯` menu proposes saving its structure
// as a route (B8). A placeholder's
// triage card and node detail break it down (B10): the pieces typed there open a proposal,
// reviewed, edited, and applied like any other (I6). Each draft is made by the host and opens
// on its review.
import type { Schema } from "@cairn/client";
import { useState } from "react";
import { useNavigate } from "react-router";

import { proposalAddress } from "../address.ts";
import { destinationOf } from "../data/edit-command.ts";
import { breakdownMutations, PieceRows, type Piece } from "../authoring/StructureEditors.tsx";
import { slugOf } from "../authoring/keys.ts";
import { journeyAuthored } from "../authoring/target.ts";
import { isPatchAnswer } from "../data/writes.ts";
import { newProposalId, proposalOf } from "../data/proposals.ts";
import type { Ready } from "../detail/model.ts";
import { RejectionView } from "../screens/RejectionView.tsx";
import { WriteStatus } from "../screens/WriteStatus.tsx";
import { Button, Input, Panel } from "../ui/kit.tsx";
import { useProposalWrite, type ProposalAsk, type ProposalWrite } from "./write.ts";

type ProposalAnswer = Schema<"ProposalAnswer">;

/**
 * Drafts a proposal as the control `edit` names and opens its review once the host has it, if
 * the control is still there: `start` sends `ask` with a proposal id minted for it. A draft
 * that got no answer is resent by the manager under the same proposal and patch ids, so one
 * that was saved after all is found rather than made twice.
 */
export function useDraftAndOpen(edit: string, label?: string) {
  const write = useProposalWrite(edit, label);
  const navigate = useNavigate();
  const start = (ask: (id: string) => ProposalAsk, after?: (answer: ProposalAnswer) => void) => {
    const id = newProposalId();
    const asked = ask(id);
    void write.run(asked).then((answer) => {
      if (answer === undefined || isPatchAnswer(answer)) {
        return;
      }
      after?.(answer);
      if (write.mounted()) {
        void navigate(proposalAddress(destinationOf(asked), proposalOf(answer)?.id ?? id));
      }
    });
  };
  return { write, start };
}

/** A proposal write's trouble under its control: a rejection with the way to review again, else what the write is doing. */
export function Problem({ write, onRebase }: { write: ProposalWrite; onRebase: () => void }) {
  return write.rejection === undefined ? <WriteStatus view={write.status} /> : <RejectionView rejection={write.rejection} onRebase={onRebase} />;
}

/** B8: the journey's structure saved as a draft of a new route, or of an existing one by its id. */
function SaveAsRoute({ ready }: { ready: Ready }) {
  const { header } = ready.journey;
  const { write, start } = useDraftAndOpen(`save-as-route:${header.id}`, header.name);
  const [route, setRoute] = useState(slugOf(header.name, "route"));
  const [name, setName] = useState(header.name);
  return (
    <span className="row" data-testid="save-as-route-flow">
      <Input aria-label="Route id" value={route} onChange={(event) => { setRoute(event.target.value); }} />
      <Input aria-label="Route name" value={name} onChange={(event) => { setName(event.target.value); }} />
      <Button disabled={write.disabled || route.trim() === "" || name.trim() === ""} onClick={() => { start((id) => ({ op: "save_as_route", journey: header.id, request: { proposal: id, route: route.trim(), name: name.trim() } })); }}>
        Propose saving it as a route
      </Button>
      <Problem write={write} onRebase={write.dismiss} />
    </span>
  );
}

/**
 * B8: the journey's save-as-route flow, opened from its `⋯` menu: it opens a proposal to
 * review, edit, and apply, and nothing changes until it is applied.
 */
export function JourneyFlow({ ready, onClose }: { ready: Ready; onClose: () => void }) {
  return (
    <Panel aria-label="Proposals" data-testid="journey-flows">
      <span className="row">
        <strong>Change it by proposal</strong>
        <span className="spacer" />
        <Button onClick={onClose}>Close</Button>
      </span>
      <SaveAsRoute ready={ready} />
    </Panel>
  );
}

/** B10: break `node` down: the pieces typed here open a proposal adding them beneath it. Shown while its owner has it open. */
export function BreakDown({ ready, node, onCancel }: { ready: Ready; node: { key: string; title: string }; onCancel: () => void }) {
  const { write, start } = useDraftAndOpen(`breakdown:${ready.journey.header.id}:${node.key}`, node.title);
  const [pieces, setPieces] = useState<Piece[]>([{ kind: "action", title: "" }]);
  const filled = pieces.filter((piece) => piece.title.trim() !== "");
  const propose = () => {
    // The pieces' keys are minted once, with the ask: the write keeps the draft, so a resend sends the same one.
    const draft = { title: `Break down ${node.title}`, destination_revision: ready.journey.revision, mutations: breakdownMutations(journeyAuthored(ready), node.key, filled) };
    start((id) => ({ op: "create", destination: { journey: ready.journey.header.id }, request: { id, draft } }));
  };
  return (
    <div className="stack" data-testid="break-down-form" data-node={node.key}>
      <PieceRows pieces={pieces} onChange={setPieces} />
      <span className="row">
        <Button onClick={() => { setPieces([...pieces, { kind: "action", title: "" }]); }}>Another piece</Button>
        <Button primary disabled={write.disabled || filled.length === 0} onClick={propose} data-testid="propose-breakdown">
          Propose the breakdown
        </Button>
        <Button onClick={onCancel}>Cancel</Button>
      </span>
      <Problem write={write} onRebase={write.dismiss} />
    </div>
  );
}
