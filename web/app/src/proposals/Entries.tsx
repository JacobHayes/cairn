// Where proposals start, on earlier screens. A journey's `⋯` menu proposes saving its structure
// as a route (B8). A placeholder's
// triage card and node detail break it down (B10): the pieces typed there open a proposal,
// reviewed, edited, and applied like any other (I6). Each is a patch that names a new proposal
// and opens on its review.
import { useState } from "react";
import { useNavigate } from "react-router";

import { proposalAddress } from "../address.ts";
import { breakdownMutations, PieceRows, type Piece } from "../authoring/StructureEditors.tsx";
import { slugOf } from "../authoring/keys.ts";
import { journeyAuthored } from "../authoring/target.ts";
import { authorCommand } from "../authoring/write.ts";
import type { Mutation } from "../authoring/graph.ts";
import type { Command } from "../data/edit-command.ts";
import { useSession, useWrite, type Write } from "../data/react.ts";
import { newProposalId, titled, type Domain } from "../data/proposals.ts";
import { newPatchId } from "../data/writes.ts";
import type { Ready } from "../detail/model.ts";
import { RejectionView } from "../screens/RejectionView.tsx";
import { WriteStatus } from "../screens/WriteStatus.tsx";
import { Button, Input, Panel } from "../ui/kit.tsx";

/**
 * Writes a patch that names a new proposal as the control `edit` names, and opens its review
 * once it is saved, if the control is still there. A write that got no answer is resent by the
 * manager under the same ids, so one that was saved after all is found rather than made twice.
 */
export function useDraftAndOpen(edit: string, label?: string) {
  const write = useWrite(edit, label);
  const navigate = useNavigate();
  const start = (destination: Domain, command: (proposal: string) => Command, after?: () => void) => {
    const id = newProposalId();
    void write.submit(command(id)).then((answer) => {
      if (answer === undefined) {
        return;
      }
      after?.();
      if (write.mounted()) {
        void navigate(proposalAddress(destination, id));
      }
    });
  };
  return { write, start };
}

/** A proposal write's trouble under its control: a rejection with the way to review again, else what the write is doing. */
export function Problem({ write, onRebase }: { write: Pick<Write, "rejection" | "status">; onRebase: () => void }) {
  return write.rejection === undefined ? <WriteStatus view={write.status} /> : <RejectionView rejection={write.rejection} onRebase={onRebase} />;
}

/** B8: the journey's structure saved as a draft of a new route, or of an existing one by its id. */
function SaveAsRoute({ ready }: { ready: Ready }) {
  const { header } = ready.journey;
  const { host } = useSession();
  const { write, start } = useDraftAndOpen(`save-as-route:${header.id}`, header.name);
  const [route, setRoute] = useState(slugOf(header.name, "route"));
  const [name, setName] = useState(header.name);
  const propose = async () => {
    const id = route.trim();
    // A route that is not there yet is created with its draft, from nothing (revision 0); one that is has its draft opened from its revision.
    // A route that cannot be read is taken as new: the server refuses a create over one that exists.
    const existing = await host.route(id).catch(() => undefined);
    const mutations: Mutation[] = [...(existing === undefined ? [{ op: "create_route" as const, name: name.trim() }] : []), { op: "open_draft", source: { save_as_route: { journey: header.id } } }];
    const patch = { id: newPatchId(), target: { route: id }, base_revision: existing?.revision ?? 0, mutations };
    start({ route: id }, (proposal) => ({ kind: "patch", patch, note: titled(`Save ${header.name} as a route`), proposal }));
  };
  return (
    <span className="row" data-testid="save-as-route-flow">
      <Input aria-label="Route id" value={route} onChange={(event) => { setRoute(event.target.value); }} />
      <Input aria-label="Route name" value={name} onChange={(event) => { setName(event.target.value); }} />
      <Button disabled={write.disabled || route.trim() === "" || name.trim() === ""} onClick={() => void propose()}>
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
    // The pieces' keys are minted once, here: the write keeps the command, so a resend sends the same one.
    const authored = journeyAuthored(ready);
    const mutations = breakdownMutations(authored, node.key, filled);
    const patch = newPatchId();
    start(authored.target, (id) => authorCommand(authored, mutations, authored.revision, patch, { id, title: `Break down ${node.title}` }));
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
