// Where proposals start, on earlier screens. A journey's `⋯` menu proposes its upgrade to a
// newer version of its route (B7, C17: one journey at a time), saving its structure as a route
// (B8), and re-linking it to a version of the route it was saved as (B9). A placeholder's
// triage card and node detail break it down (B10): the pieces typed there open a proposal,
// reviewed, edited, and applied like any other (I6). Each draft is made by the host and opens
// on its review.
import type { Schema } from "@cairn/client";
import { useRef, useState } from "react";
import { useNavigate } from "react-router";

import { breakdownMutations } from "../authoring/StructureEditors.tsx";
import { slugOf } from "../authoring/keys.ts";
import { Picker } from "../authoring/parts.tsx";
import { journeyAuthored } from "../authoring/target.ts";
import type { NodeKind } from "../authoring/graph.ts";
import { routeIndex } from "../data/reads.ts";
import { newProposalId, proposalOf, type ProposalWritten } from "../data/proposals.ts";
import { newPatchId } from "../data/writes.ts";
import { useLive } from "../data/react.ts";
import type { ProposalHost } from "../data/proposals.ts";
import type { Ready } from "../detail/model.ts";
import { RejectionView } from "../screens/RejectionView.tsx";
import { Button, Field, Panel } from "../ui/kit.tsx";
import { PIECE_KINDS, attemptFor, type Attempt } from "./model.ts";
import { proposalPath } from "./read.ts";
import { useProposalWrite, type WriteProblem } from "./write.ts";

type ProposalAnswer = Schema<"ProposalAnswer">;

/**
 * Drafts a proposal through `call` and opens its review once the host has it. An attempt
 * whose answer never came (I6, H5) is sent again under the same proposal and patch ids while
 * it asks the same, so a draft that was saved after all is found rather than made twice.
 */
function useDraftAndOpen() {
  const write = useProposalWrite();
  const navigate = useNavigate();
  const unanswered = useRef<Attempt | undefined>(undefined);
  const start = (asked: unknown, call: (proposals: ProposalHost, patchId: string, id: string) => Promise<ProposalWritten<ProposalAnswer>>) => {
    const attempt = attemptFor(unanswered.current, JSON.stringify(asked), () => ({ id: newProposalId(), patchId: newPatchId() }));
    unanswered.current = undefined;
    void write.run(async (proposals, patchId) => {
      const written = await call(proposals, patchId, attempt.id);
      if (written.outcome === "failed") {
        unanswered.current = attempt;
      }
      return written;
    }, attempt.patchId).then((answer) => {
      if (answer !== undefined) {
        void navigate(proposalPath(proposalOf(answer)?.id ?? attempt.id));
      }
    });
  };
  return { write, start };
}

function Problem({ problem, onDismiss }: { problem: WriteProblem | undefined; onDismiss: () => void }) {
  if (problem === undefined) {
    return null;
  }
  return "failure" in problem ? (
    <span className="author-problem" role="alert" data-testid="flow-problem">
      Not drafted: {problem.failure.message}
    </span>
  ) : (
    <RejectionView rejection={problem.rejection} onRebase={onDismiss} />
  );
}

/** B7: the versions newer than the journey's, from the route index's latest. */
function Upgrade({ ready, latest }: { ready: Ready; latest: number | undefined }) {
  const lineage = ready.journey.header.lineage ?? undefined;
  const { write, start } = useDraftAndOpen();
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
      <Button primary disabled={write.disabled || chosen === undefined} onClick={() => { start(["upgrade", ready.journey.header.id, chosen], (proposals, patchId, id) => proposals.upgrade(ready.journey.header.id, { patch_id: patchId, proposal: id, to: chosen ?? lineage.version })); }}>
        Propose the upgrade
      </Button>
      <Problem problem={write.problem} onDismiss={write.dismiss} />
    </span>
  );
}

/** B8: the journey's structure saved as a draft of a route: its own route, or a new one. */
function SaveAsRoute({ ready }: { ready: Ready }) {
  const { header } = ready.journey;
  const { write, start } = useDraftAndOpen();
  const [route, setRoute] = useState(header.lineage?.route ?? slugOf(header.name, "route"));
  const [name, setName] = useState(header.name);
  return (
    <span className="row" data-testid="save-as-route-flow">
      <Field aria-label="Route id" value={route} onChange={(event) => { setRoute(event.target.value); }} />
      <Field aria-label="Route name" value={name} onChange={(event) => { setName(event.target.value); }} />
      <Button disabled={write.disabled || route.trim() === "" || name.trim() === ""} onClick={() => { start(["save", header.id, route.trim(), name.trim()], (proposals, patchId, id) => proposals.saveAsRoute(header.id, { patch_id: patchId, proposal: id, route: route.trim(), name: name.trim() })); }}>
        Propose saving it as a route
      </Button>
      <Problem problem={write.problem} onDismiss={write.dismiss} />
    </span>
  );
}

/** B9: the journey re-linked to a published version of a route. */
function Relink({ ready, routes }: { ready: Ready; routes: readonly Schema<"RouteSummary">[] }) {
  const { write, start } = useDraftAndOpen();
  const published = routes.filter((each) => each.latest_version != null);
  const [route, setRoute] = useState("");
  const [version, setVersion] = useState("");
  const latest = published.find((each) => each.header.id === route)?.latest_version ?? undefined;
  const versions = latest === undefined ? [] : Array.from({ length: latest }, (_, at) => at + 1);
  const chosen = version === "" ? latest : Number(version);
  return (
    <span className="row" data-testid="relink-flow">
      <Picker aria-label="Re-link to route" value={route} none="Choose a route" options={published.map((each) => ({ value: each.header.id, label: each.header.name }))} onChange={(event) => { setRoute(event.target.value); setVersion(""); }} />
      {versions.length === 0 ? null : <Picker aria-label="Re-link to version" value={String(chosen)} options={versions.map((each) => ({ value: String(each), label: `version ${String(each)}` }))} onChange={(event) => { setVersion(event.target.value); }} />}
      <Button disabled={write.disabled || route === "" || chosen === undefined} onClick={() => { start(["relink", ready.journey.header.id, route, chosen], (proposals, patchId, id) => proposals.relink(ready.journey.header.id, { patch_id: patchId, proposal: id, lineage: { route, version: chosen ?? 1 } })); }}>
        Propose the re-link
      </Button>
      <Problem problem={write.problem} onDismiss={write.dismiss} />
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
  const list = routes.status === "ready" ? routes.value : [];
  const lineage = ready.journey.header.lineage ?? undefined;
  const latest = lineage === undefined ? undefined : (list.find((each) => each.header.id === lineage.route)?.latest_version ?? undefined);
  return (
    <Panel aria-label="Proposals" data-testid="journey-flows">
      <span className="row">
        <strong>Change it by proposal</strong>
        <span className="spacer" />
        <Button onClick={onClose}>Close</Button>
      </span>
      <span className="muted small">It opens a proposal to review, edit, and apply; nothing changes until it is applied.</span>
      {flow === "upgrade" ? <Upgrade ready={ready} latest={latest} /> : null}
      {flow === "save" ? <SaveAsRoute ready={ready} /> : null}
      {flow === "relink" ? <Relink ready={ready} routes={list} /> : null}
    </Panel>
  );
}

interface Piece {
  kind: NodeKind;
  title: string;
}

/** B10: break `node` down: the pieces typed here open a proposal adding them beneath it. */
export function BreakDown({ ready, node, startOpen = false, onCancel }: { ready: Ready; node: { key: string; title: string }; startOpen?: boolean; onCancel?: () => void }) {
  const { write, start } = useDraftAndOpen();
  const [pieces, setPieces] = useState<Piece[] | undefined>(startOpen ? [{ kind: "action", title: "" }] : undefined);
  const drafted = useRef<{ asked: string; draft: Schema<"ProposalDraft"> } | undefined>(undefined);
  if (pieces === undefined) {
    return (
      <Button onClick={() => { setPieces([{ kind: "action", title: "" }]); }} data-testid="break-down">
        Break down
      </Button>
    );
  }
  const filled = pieces.filter((piece) => piece.title.trim() !== "");
  const set = (at: number, piece: Partial<Piece>) => {
    setPieces(pieces.map((each, index) => (index === at ? { ...each, ...piece } : each)));
  };
  const propose = () => {
    // The pieces' keys are minted once per attempt asked, so a resubmission sends the same draft.
    const asked = ["breakdown", ready.journey.header.id, node.key, ready.journey.revision, filled];
    const draft = drafted.current?.asked === JSON.stringify(asked) ? drafted.current.draft : { title: `Break down ${node.title}`, destination_revision: ready.journey.revision, mutations: breakdownMutations(journeyAuthored(ready), node.key, filled) };
    drafted.current = { asked: JSON.stringify(asked), draft };
    start(asked, (proposals, patchId, id) => proposals.create({ journey: ready.journey.header.id }, { patch_id: patchId, id, draft }));
  };
  return (
    <div className="stack" data-testid="break-down-form" data-node={node.key}>
      {pieces.map((piece, at) => (
        <span key={at} className="row" data-testid="piece">
          <Picker aria-label="Kind of piece" value={piece.kind} options={PIECE_KINDS.map((kind) => ({ value: kind, label: kind }))} onChange={(event) => { set(at, { kind: event.target.value as NodeKind }); }} />
          <Field aria-label="Piece title" placeholder="What it is" value={piece.title} onChange={(event) => { set(at, { title: event.target.value }); }} />
        </span>
      ))}
      <span className="row">
        <Button onClick={() => { setPieces([...pieces, { kind: "action", title: "" }]); }}>Another piece</Button>
        <Button primary disabled={write.disabled || filled.length === 0} onClick={propose} data-testid="propose-breakdown">
          Propose the breakdown
        </Button>
        <Button onClick={() => { setPieces(undefined); onCancel?.(); }}>Cancel</Button>
      </span>
      <Problem problem={write.problem} onDismiss={write.dismiss} />
    </div>
  );
}
