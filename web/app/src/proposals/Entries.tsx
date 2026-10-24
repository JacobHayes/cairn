// Where proposals start, on earlier screens. A journey's `⋯` menu proposes its upgrade to a
// newer version of its route (B7, C17: one journey at a time), saving its structure as a route
// (B8), and re-linking it to a version of the route it was saved as (B9). A placeholder's
// triage card and node detail break it down (B10): the pieces typed there open a proposal,
// reviewed, edited, and applied like any other (I6). Each draft is made by the host and opens
// on its review.
import type { Schema } from "@cairn/client";
import { useState } from "react";
import { useNavigate } from "react-router";

import { proposalAddress } from "../address.ts";
import { destinationOf } from "../data/edit-record.ts";
import { breakdownMutations, PieceRows, type Piece } from "../authoring/StructureEditors.tsx";
import { slugOf } from "../authoring/keys.ts";
import { Picker } from "../authoring/parts.tsx";
import { journeyAuthored } from "../authoring/target.ts";
import { isSegment, routeIndex } from "../data/reads.ts";
import { isPatchAnswer } from "../data/writes.ts";
import { newProposalId, proposalOf } from "../data/proposals.ts";
import { valueOf } from "../data/keyed-read.ts";
import { useLive } from "../data/react.ts";
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

/** B7: the versions newer than the journey's, from the route index's latest. */
function Upgrade({ ready, latest }: { ready: Ready; latest: number | undefined }) {
  const lineage = ready.journey.header.lineage ?? undefined;
  const { write, start } = useDraftAndOpen(`upgrade:${ready.journey.header.id}`, ready.journey.header.name);
  const newer = lineage === undefined || latest === undefined ? [] : Array.from({ length: latest - lineage.version }, (_, at) => lineage.version + at + 1);
  const [to, setTo] = useState("");
  if (lineage === undefined) {
    return null;
  }
  if (newer.length === 0) {
    return <span className="muted small" data-testid="upgrade-none">On the latest version of its route: nothing to upgrade to.</span>;
  }
  const chosen = to === "" ? newer.at(-1) : Number(to);
  return (
    <span className="row" data-testid="upgrade-flow">
      <Picker aria-label="Upgrade to version" value={String(chosen)} options={newer.map((version) => ({ value: String(version), label: `version ${String(version)}` }))} onChange={(event) => { setTo(event.target.value); }} />
      <Button primary disabled={write.disabled || chosen === undefined} onClick={() => { start((id) => ({ op: "upgrade", journey: ready.journey.header.id, request: { proposal: id, to: chosen ?? lineage.version } })); }}>
        Propose the upgrade to v{chosen ?? lineage.version}
      </Button>
      <Problem write={write} onRebase={write.dismiss} />
    </span>
  );
}

/** B8: the journey's structure saved as a draft of a route: its own route, or a new one. */
function SaveAsRoute({ ready }: { ready: Ready }) {
  const { header } = ready.journey;
  const { write, start } = useDraftAndOpen(`save-as-route:${header.id}`, header.name);
  const [route, setRoute] = useState(header.lineage?.route ?? slugOf(header.name, "route"));
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

/** B9: the journey re-linked to a published version of a route. */
function Relink({ ready, routes }: { ready: Ready; routes: readonly Schema<"RouteSummary">[] }) {
  const { write, start } = useDraftAndOpen(`relink:${ready.journey.header.id}`, ready.journey.header.name);
  const published = routes.filter((each) => each.latest_version != null && !isSegment(each));
  const [route, setRoute] = useState("");
  const [version, setVersion] = useState("");
  const latest = published.find((each) => each.header.id === route)?.latest_version ?? undefined;
  const versions = latest === undefined ? [] : Array.from({ length: latest }, (_, at) => at + 1);
  const chosen = version === "" ? latest : Number(version);
  return (
    <span className="row" data-testid="relink-flow">
      <Picker aria-label="Re-link to route" value={route} none="Choose a route" options={published.map((each) => ({ value: each.header.id, label: each.header.name }))} onChange={(event) => { setRoute(event.target.value); setVersion(""); }} />
      {versions.length === 0 ? null : <Picker aria-label="Re-link to version" value={String(chosen)} options={versions.map((each) => ({ value: String(each), label: `version ${String(each)}` }))} onChange={(event) => { setVersion(event.target.value); }} />}
      <Button disabled={write.disabled || route === "" || chosen === undefined} onClick={() => { start((id) => ({ op: "relink", journey: ready.journey.header.id, request: { proposal: id, lineage: { route, version: chosen ?? 1 } } })); }}>
        Propose the re-link
      </Button>
      <Problem write={write} onRebase={write.dismiss} />
    </span>
  );
}

/** The journey `⋯` menu's proposal flows (B7, B8, B9). */
export type JourneyFlowName = "upgrade" | "save" | "relink";

/**
 * B7, B8, B9: one of the journey's proposal flows, opened from its `⋯` menu: each opens a
 * proposal to review, edit, and apply, and nothing changes until it is applied.
 */
export function JourneyFlow({ ready, flow, onClose }: { ready: Ready; flow: JourneyFlowName; onClose: () => void }) {
  const routes = useLive("routes", routeIndex).view;
  const list = valueOf(routes) ?? [];
  const lineage = ready.journey.header.lineage ?? undefined;
  const latest = lineage === undefined ? undefined : (list.find((each) => each.header.id === lineage.route)?.latest_version ?? undefined);
  return (
    <Panel aria-label="Proposals" data-testid="journey-flows">
      <span className="row">
        <strong>Change it by proposal</strong>
        <span className="spacer" />
        <Button onClick={onClose}>Close</Button>
      </span>
      {flow === "upgrade" ? <Upgrade ready={ready} latest={latest} /> : null}
      {flow === "save" ? <SaveAsRoute ready={ready} /> : null}
      {flow === "relink" ? <Relink ready={ready} routes={list} /> : null}
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
