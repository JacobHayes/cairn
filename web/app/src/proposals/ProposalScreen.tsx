// C14: proposal review, for every destination (a journey, a route, the deployment, or one the
// proposal creates at revision 0): the proposal as a diff over the canvas and as lists (what
// it changes, its review items, its mutations), editable item by item, previewed as edited,
// then saved, applied, or discarded (I6). A journey's preview is the engine's in the derive
// worker over the edited draft, with the frontier after (ARCHITECTURE, Web UI: Previews);
// any other destination's is the host's preview of the proposal as saved. A stale proposal
// shows what moved and is refreshed and reviewed again before it applies; apply sends the
// revision the reviewer confirmed reviewing, and is offered only when nothing blocks it.
import type { Schema } from "@cairn/client";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Link, useParams } from "react-router";

import { treeOf, type Graph, type Kind, type Role } from "../authoring/graph.ts";
import { canvasPath, DEFAULT_VIEW } from "../canvas/settings.ts";
import { PREVIEW_SETTLE_MS } from "../authoring/write.ts";
import type { Deployment, RouteVersion } from "../data/host.ts";
import { useDraft } from "../data/drafts.ts";
import type { ProposalReview } from "../data/proposals.ts";
import { useDeployment, useJourney, useLive, useSession, useViewer } from "../data/react.ts";
import type { Ready } from "../detail/model.ts";
import { ShortfallView } from "../detail/ShortfallView.tsx";
import { overviewPath } from "../journeys/address.ts";
import { routeDetailPath } from "../routes/address.ts";
import { RejectionView } from "../screens/RejectionView.tsx";
import { Badge, Button, Panel } from "../ui/kit.tsx";
import { Markdown } from "../ui/markdown.tsx";
import { ChangeList } from "./Changes.tsx";
import { ItemList } from "./Items.tsx";
import {
  applyBlockers,
  diffMarks,
  graphDiff,
  itemNode,
  reviewedAfter,
  unionGraph,
  unresolvedHere,
  withMutationsAdded,
  type Blocker,
  type GraphDiff,
  type Proposal,
  type ProposalDraft,
  type ProposalPreview,
} from "./model.ts";
import { ProposalCanvas } from "./ProposalCanvas.tsx";
import { proposalReview } from "./read.ts";
import { StalePanel } from "./Stale.tsx";
import { fieldName, namesOf, type Names } from "./words.ts";
import { useProposalWrite, type ProposalWrite, type WriteProblem } from "./write.ts";
import "./proposals.css";

/** The reviewer's edits not yet saved, and the proposal revision they started from (H5). */
interface Kept {
  base: number;
  draft: ProposalDraft;
}

const NOTHING_COMPARED: GraphDiff = { added: [], removed: [], changed: {}, edgesAdded: [], edgesRemoved: [] };

const BLOCKER_WORDS: Record<Blocker, string> = {
  not_open: "It is no longer open.",
  editing: "An edit has a problem: fix it or drop the change.",
  unsaved: "Save or drop your edits first.",
  stale: "Its destination moved: refresh it first.",
  unresolved: "An item still needs a choice.",
  invalid: "As it stands it breaks a rule; see the violations.",
  unreviewed: "Confirm you have reviewed it as it stands now.",
  no_preview: "Waiting for its preview.",
};

/** The route versions a journey proposal's mutations read: an upgrade's target and the journey's own, a re-link's new version. */
function versionsRead(ready: Ready, draft: ProposalDraft): Schema<"Lineage">[] {
  const own = ready.journey.header.lineage ?? undefined;
  return (draft.mutations ?? []).flatMap((mutation) => {
    if (mutation.op === "upgrade" && own !== undefined) {
      return [own, { route: own.route, version: mutation.to }];
    }
    if (mutation.op === "relink") {
      return own === undefined ? [mutation.lineage] : [own, mutation.lineage];
    }
    return [];
  });
}

type LocalPreview = { preview: ProposalPreview } | { error: string };

/** C14: the edited proposal previewed by the engine over the journey the derive worker holds. */
function useLocalPreview(ready: Ready | undefined, proposal: Proposal, draft: ProposalDraft): LocalPreview | undefined {
  const { deriver, host } = useSession();
  const { viewer } = useViewer();
  // Only an open proposal is previewed here: one applied or discarded has nothing left to apply.
  const previewed = proposal.status === "open" ? ready : undefined;
  const asked = previewed === undefined ? "" : JSON.stringify([previewed.key, proposal.revision, draft]);
  const [answered, setAnswered] = useState<{ for: string; preview: LocalPreview } | undefined>();
  useEffect(() => {
    if (previewed === undefined) {
      return;
    }
    let live = true;
    const timer = setTimeout(() => {
      const read = async (): Promise<LocalPreview> => {
        const lineages = versionsRead(previewed, draft);
        const versions: RouteVersion[] = await Promise.all(lineages.map((lineage) => host.routeVersion(lineage.route, lineage.version)));
        const request = { proposal: { ...proposal, draft }, versions, at: new Date().toISOString(), actor: { user: viewer?.user ?? "u_local" } };
        return { preview: await deriver.preview(previewed.journey.header.id, request) };
      };
      read().then(
        (preview) => {
          if (live) {
            setAnswered({ for: asked, preview });
          }
        },
        (thrown: unknown) => {
          if (live) {
            setAnswered({ for: asked, preview: { error: thrown instanceof Error ? thrown.message : String(thrown) } });
          }
        },
      );
    }, PREVIEW_SETTLE_MS);
    return () => {
      live = false;
      clearTimeout(timer);
    };
    // `asked` names the journey's derivation, the proposal's revision, and the draft.
  }, [asked]);
  return answered?.for === asked ? answered.preview : undefined;
}

/** C14: the diff as a list: nodes added, changed (with their fields), and removed, and edges. */
function DiffList({ diff, names, selected, onPick }: { diff: GraphDiff; names: Names; selected: string | undefined; onPick: (key: string) => void }) {
  const row = (key: string, words: string, status: string) => (
    <li key={`${status}:${key}`} data-testid="diff-node" data-node={key} data-status={status} className={key === selected ? "proposal-picked" : undefined}>
      <button type="button" className="link" onClick={() => { onPick(key); }}>
        {names.node(key)}
      </button>{" "}
      <span className="muted">{words}</span>
    </li>
  );
  const changed = Object.entries(diff.changed);
  if (diff.added.length + diff.removed.length + changed.length + diff.edgesAdded.length + diff.edgesRemoved.length === 0) {
    return <p className="muted" data-testid="diff-empty">As it stands, it changes nothing in the graph.</p>;
  }
  return (
    <ul className="detail-list" data-testid="diff-list">
      {diff.added.map((key) => row(key, "added", "added"))}
      {changed.map(([key, fields]) => row(key, `changed: ${fields.map(fieldName).join(", ")}`, "changed"))}
      {diff.removed.map((key) => row(key, "removed", "removed"))}
      {diff.edgesAdded.map((edge) => (
        <li key={`+${edge.node}>${edge.requires}`} data-testid="diff-edge" data-status="added">
          {names.node(edge.node)} now requires {names.node(edge.requires)}
        </li>
      ))}
      {diff.edgesRemoved.map((edge) => (
        <li key={`-${edge.node}>${edge.requires}`} data-testid="diff-edge" data-status="removed">
          {names.node(edge.node)} no longer requires {names.node(edge.requires)}
        </li>
      ))}
    </ul>
  );
}

/**
 * Why there is no diff: the candidate breaks a rule, so there is no graph after; or the
 * destination is not a graph (the deployment's entities), so there is none to draw.
 */
function NoGraphAfter({ preview }: { preview: ProposalPreview | undefined }) {
  return (preview?.violations ?? []).length > 0 ? (
    <p className="muted" data-testid="diff-unknown">Not known until it no longer breaks a rule; see the violations.</p>
  ) : (
    <p className="muted" data-testid="diff-graphless">It changes the deployment's entities, not a graph; its changes are listed beside.</p>
  );
}

/** C14: a journey's frontier after, in rank order, what joins it marked, and what leaves it. */
function FrontierAfter({ ready, preview, names }: { ready: Ready | undefined; preview: ProposalPreview; names: Names }) {
  const after = preview.frontier ?? [];
  const before = new Set(ready?.derived.frontier ?? []);
  const leaving = [...before].filter((key) => !after.includes(key));
  return (
    <Panel aria-label="The frontier after" data-testid="frontier-after">
      <strong>What can be acted on after it applies</strong>
      {after.length === 0 ? <span className="muted">Nothing.</span> : null}
      <ol className="detail-list">
        {after.map((key) => (
          <li key={key} data-testid="frontier-node" data-node={key} data-status={before.has(key) ? "kept" : "new"}>
            {names.node(key)} {before.has(key) || ready === undefined ? null : <Badge tone="good">new</Badge>}
          </li>
        ))}
      </ol>
      {leaving.length === 0 ? null : <span className="muted" data-testid="frontier-leaving">No longer on it: {leaving.map((key) => names.node(key)).join(", ")}</span>}
    </Panel>
  );
}

/** A15, F5: the candidate's violations; a contradictory chain offers 5.1's resolution moves, each added to the proposal. */
function Violations({ preview, ready, names, onMove }: { preview: ProposalPreview; ready: Ready | undefined; names: Names; onMove: ((move: Schema<"Mutation">) => void) | undefined }) {
  const violations = preview.violations ?? [];
  if (violations.length === 0) {
    return null;
  }
  const chains = violations.flatMap((violation) => violation.chains?.chains ?? []);
  return (
    <div className="callout callout-bad stack" role="alert" data-testid="proposal-violations">
      <strong>As it stands, applying it would be refused:</strong>
      <ul className="detail-list">
        {violations.map((violation, at) => {
          const subject = violation.at.subject;
          const node = subject != null && typeof subject === "object" && "node" in subject ? subject.node : undefined;
          return (
            <li key={at} data-testid="violation" data-code={violation.code} data-node={node}>
              {node === undefined ? null : <strong>{names.node(node)}: </strong>}
              {violation.message}
            </li>
          );
        })}
      </ul>
      {ready === undefined ? null : chains.map((short, at) => <ShortfallView key={at} view={ready} short={short} disabled={onMove === undefined} onMove={onMove} />)}
    </div>
  );
}

function Problem({ problem, onDismiss }: { problem: WriteProblem | undefined; onDismiss: () => void }) {
  if (problem === undefined) {
    return null;
  }
  if ("failure" in problem) {
    return (
      <p className="callout callout-bad" role="alert" data-testid="proposal-problem">
        Not done: {problem.failure.message} <Button onClick={onDismiss}>Dismiss</Button>
      </p>
    );
  }
  return <RejectionView rejection={problem.rejection} onRebase={onDismiss} />;
}

/** Where the proposal goes, as a link. */
function DestinationLink({ proposal }: { proposal: Proposal }) {
  const destination = proposal.destination;
  if (destination === "deployment") {
    return <span>the deployment</span>;
  }
  if ("journey" in destination) {
    return proposal.draft.destination_revision === 0 ? <span>a new journey, {destination.journey}</span> : <Link to={overviewPath(destination.journey)}>the journey {destination.journey}</Link>;
  }
  return <Link to={routeDetailPath(destination.route)}>the route {destination.route}</Link>;
}

/** I6: the controls that save, confirm, apply, or discard it, with what blocks an apply. */
function Footer({ proposal, kept, editing, blockers, reviewed, write, onSave, onDrop, onMark, onApply, onDiscard }: {
  proposal: Proposal;
  kept: Kept | undefined;
  editing: boolean;
  blockers: Blocker[];
  reviewed: number | undefined;
  write: ProposalWrite;
  onSave: () => void;
  onDrop: () => void;
  onMark: (marked: boolean) => void;
  onApply: () => void;
  onDiscard: () => void;
}) {
  if (proposal.status !== "open") {
    return null;
  }
  return (
    <Panel aria-label="Apply or discard" data-testid="proposal-footer">
      {kept === undefined ? null : (
        <span className="row" data-testid="unsaved">
          <strong>Your edits are not saved yet.</strong>
          {kept.base === proposal.revision ? null : <span className="muted">The proposal changed since you started editing.</span>}
          <Button primary disabled={write.disabled || editing} onClick={onSave} data-testid="save-proposal">
            Save the edits
          </Button>
          <Button onClick={onDrop}>Drop them</Button>
        </span>
      )}
      <label className="row">
        <input type="checkbox" disabled={kept !== undefined} checked={reviewed === proposal.revision} onChange={(event) => { onMark(event.target.checked); }} data-testid="reviewed" />
        I have reviewed this proposal as it stands (revision {proposal.revision})
      </label>
      <span className="row">
        <Button primary disabled={write.disabled || blockers.length > 0} onClick={onApply} data-testid="apply-proposal" data-blockers={blockers.join(" ")}>
          Apply
        </Button>
        <Button disabled={write.disabled} onClick={onDiscard} data-testid="discard-proposal">
          Discard the proposal
        </Button>
      </span>
      {blockers.length === 0 ? null : (
        <ul className="detail-list muted" data-testid="blockers">
          {blockers.map((blocker) => (
            <li key={blocker} data-testid="blocker" data-blocker={blocker}>
              {BLOCKER_WORDS[blocker]}
            </li>
          ))}
        </ul>
      )}
    </Panel>
  );
}

interface BodyProps {
  review: ProposalReview;
  refetch: () => void;
  /** The journey the proposal is for, derived, when it is one that exists. */
  ready: Ready | undefined;
  /** The destination's graph as it is (none for one the proposal creates). */
  before: Graph | undefined;
  deployment: Deployment;
}

/** The editors holding a value with a problem, by name, and how one reports it. */
function useInvalidEditors() {
  const [invalid, setInvalidSet] = useState<ReadonlySet<string>>(new Set());
  const setInvalid = useCallback((editor: string, held: boolean) => {
    setInvalidSet((current) => {
      if (current.has(editor) === held) {
        return current;
      }
      const next = new Set(current);
      if (held) {
        next.add(editor);
      } else {
        next.delete(editor);
      }
      return next;
    });
  }, []);
  return { invalid, setInvalid };
}

/** I6: the reviewer's edits, their confirmation, and the writes that save, refresh, apply, or discard. */
function useReviewing(review: ProposalReview, refetch: () => void) {
  const proposal = review.proposal;
  const id = proposal.id;
  const write = useProposalWrite();
  const [kept, setKept] = useDraft<Kept>(`proposal:${id}`);
  const [reviewed, setReviewed] = useDraft<number>(`proposal-reviewed:${id}`);
  const { invalid, setInvalid } = useInvalidEditors();
  const saved = (answer: Schema<"ProposalAnswer"> | undefined, event: "saved" | "refreshed") => {
    if (answer === undefined) {
      return;
    }
    if (answer.outcome !== "already_saved") {
      setReviewed(reviewedAfter(reviewed, { event, proposal: answer.proposal }));
    }
    refetch();
  };
  return {
    write,
    kept,
    reviewed,
    editing: invalid.size > 0,
    setInvalid,
    blockers: applyBlockers({ proposal, reviewed, dirty: kept !== undefined, editing: invalid.size > 0, stale: review.stale != null, preview: review.preview }),
    edit: (next: ProposalDraft) => {
      setKept({ base: kept?.base ?? proposal.revision, draft: next });
    },
    drop: () => {
      setKept(undefined);
    },
    /** H5: the reviewer's edits kept, to be saved over the proposal as it now stands. */
    rebase: () => {
      if (kept !== undefined) {
        setKept({ base: proposal.revision, draft: kept.draft });
      }
      write.dismiss();
    },
    mark: (marked: boolean) => {
      setReviewed(marked ? reviewedAfter(reviewed, { event: "marked", revision: proposal.revision }) : undefined);
    },
    save: () => {
      if (kept !== undefined && invalid.size === 0) {
        void write.run((proposals, patchId) => proposals.edit(id, { patch_id: patchId, base_revision: kept.base, draft: kept.draft })).then((answer) => {
          if (answer !== undefined) {
            setKept(undefined);
          }
          saved(answer, "saved");
        });
      }
    },
    refresh: () => {
      void write.run((proposals, patchId) => proposals.refresh(id, { patch_id: patchId, base_revision: proposal.revision })).then((answer) => {
        saved(answer, "refreshed");
      });
    },
    apply: () => {
      if (reviewed !== undefined) {
        void write.run((proposals, patchId) => proposals.apply(id, { patch_id: patchId, reviewed_revision: reviewed })).then(refetch);
      }
    },
    discard: () => {
      void write.run((proposals, patchId) => proposals.discard(id, { patch_id: patchId, base_revision: proposal.revision })).then(refetch);
    },
  };
}

/** The proposal's title, status, destination, and author. */
function Header({ proposal }: { proposal: Proposal }) {
  const journey = proposal.destination !== "deployment" && "journey" in proposal.destination ? proposal.destination.journey : undefined;
  return (
    <section className="stack" aria-label={proposal.draft.title}>
      <div className="row">
        <h1 className="title" data-testid="proposal-title">{proposal.draft.title}</h1>
        <Badge tone={proposal.status === "open" ? "plain" : proposal.status === "applied" ? "good" : "warn"} data-testid="proposal-status" data-status={proposal.status}>
          {proposal.status}
        </Badge>
      </div>
      <span className="muted">
        A proposal for <DestinationLink proposal={proposal} />, drafted by {proposal.created_by}
        {proposal.proposing_agent == null ? "" : ` with the agent ${proposal.proposing_agent}`} at its revision {proposal.draft.destination_revision}; this is its revision {proposal.revision}.
      </span>
      {proposal.status === "applied" && journey !== undefined ? <Link to={canvasPath(journey, DEFAULT_VIEW)} data-testid="applied-journey">Open the journey</Link> : null}
      {proposal.draft.description == null ? null : <Markdown text={proposal.draft.description} />}
    </section>
  );
}

/** What a review shows beside its canvas, and what its lists edit. */
interface Shown {
  draft: ProposalDraft;
  /** The graph after is known, so the diff and the frontier after can be shown. */
  known: boolean;
  preview: ProposalPreview | undefined;
  diff: GraphDiff;
  names: Names;
  tree: ReturnType<typeof treeOf>;
  structure: Graph;
  today: string;
  editable: boolean;
  selected: string | undefined;
}

function Columns({ shown, current, ready, deployment, journey, edit, setInvalid, onPick }: { shown: Shown; current: boolean; setInvalid: (editor: string, invalid: boolean) => void; ready: Ready | undefined; deployment: Deployment; journey: string | undefined; edit: (next: ProposalDraft) => void; onPick: (key: string) => void }) {
  const { draft, known, preview, diff, names, tree, structure, today, editable, selected } = shown;
  const roles: readonly Role[] = structure.roles ?? [];
  const kinds: readonly Kind[] = structure.participation_kinds ?? [];
  const unresolved = current ? new Map((preview?.unresolved ?? []).map((item) => [item.item, item.reason])) : unresolvedHere(draft.items ?? []);
  const placeholder = (draft.mutations ?? []).find((mutation) => mutation.op === "add_node")?.node.parent ?? undefined;
  return (
    <div className="proposal-columns">
      <div className="stack">
        <Panel aria-label="What it changes">
          <strong>What it changes</strong>
          {known ? <DiffList diff={diff} names={names} selected={selected} onPick={onPick} /> : <NoGraphAfter preview={preview} />}
        </Panel>
        {preview === undefined || journey === undefined || !known ? null : <FrontierAfter ready={ready} preview={preview} names={names} />}
        {preview === undefined ? null : <Violations preview={preview} ready={ready} names={names} onMove={editable ? (move) => { edit(withMutationsAdded(draft, [move])); } : undefined} />}
      </div>
      <div className="stack">
        <Panel aria-label="Review items">
          <strong>To review</strong>
          <ItemList context={{ draft, edit, names, roles, kinds, unresolved, editable, selected }} itemNode={itemNode} />
        </Panel>
        <Panel aria-label="Changes">
          <strong>Its changes</strong>
          <ChangeList context={{ draft, edit, names, tree, roles, deployment, today, ready, editable, selected, defaultParent: placeholder, setInvalid }} />
        </Panel>
      </div>
    </div>
  );
}

function ReviewBody({ review, refetch, ready, before, deployment }: BodyProps) {
  const proposal = review.proposal;
  const reviewing = useReviewing(review, refetch);
  const { kept } = reviewing;
  const [selected, setSelected] = useState<string | undefined>();
  const draft = kept?.draft ?? proposal.draft;
  const editable = proposal.status === "open" && !reviewing.write.disabled;
  const local = useLocalPreview(ready, proposal, draft);
  // A journey's preview follows the edits; any other's is the host's, of the proposal as saved.
  const previewOfEdits = local !== undefined && "preview" in local;
  const preview = previewOfEdits ? local.preview : review.preview;
  const after = preview.graph ?? undefined;
  // A candidate that breaks a rule has no graph after; that is not an empty graph, so nothing is compared.
  const known = after !== undefined;
  const diff = useMemo(() => (known ? graphDiff(before, after) : NOTHING_COMPARED), [known, before, after]);
  const marks = useMemo(() => diffMarks(diff, draft.items ?? []), [diff, draft.items]);
  const graphs = useMemo(() => (known ? [unionGraph(before, after), after, ...(before === undefined ? [] : [before])] : before === undefined ? [] : [before]), [known, before, after]);
  const structure = useMemo<Graph>(() => after ?? before ?? { nodes: [] }, [after, before]);
  const tree = useMemo(() => treeOf(structure), [structure]);
  const added = useMemo<Graph>(() => ({ nodes: (draft.mutations ?? []).flatMap((mutation) => (mutation.op === "add_node" ? [mutation.node] : [])) }), [draft.mutations]);
  const names = useMemo(() => namesOf([added, before, after], deployment.entities ?? []), [added, before, after, deployment]);
  const today = ready?.key.today ?? new Date().toISOString().slice(0, 10);
  const journey = proposal.destination !== "deployment" && "journey" in proposal.destination ? proposal.destination.journey : undefined;
  const shown: Shown = { draft, known, preview, diff, names, tree, structure, today, editable, selected };
  return (
    <div className="stack proposal-page" data-testid="proposal" data-proposal={proposal.id} data-status={proposal.status} data-revision={proposal.revision}>
      <Header proposal={proposal} />
      <StalePanel review={review} disabled={!editable} onRefresh={reviewing.refresh} />
      <Problem problem={reviewing.write.problem} onDismiss={reviewing.rebase} />
      {local !== undefined && "error" in local ? <p className="callout callout-bad">No preview: {local.error}</p> : null}
      {kept !== undefined && !previewOfEdits ? <p className="muted" data-testid="preview-of-saved">{ready === undefined ? "The preview shows the proposal as saved: save your edits to preview them." : "Previewing your edits..."}</p> : null}
      {known || before === undefined || (preview.violations ?? []).length === 0 ? null : <p className="muted" data-testid="no-graph-after">As it stands it breaks a rule, so there is no graph after to compare: the canvas shows the graph as it is, with the items marked.</p>}
      {graphs.length === 0 ? null : <ProposalCanvas domain={proposal.id} graphs={graphs} marks={marks} deployment={deployment} today={today} selected={selected} onPick={setSelected} />}
      <Columns shown={shown} current={previewOfEdits || kept === undefined} ready={ready} deployment={deployment} journey={journey} edit={reviewing.edit} setInvalid={reviewing.setInvalid} onPick={setSelected} />
      <Footer
        proposal={proposal}
        kept={kept}
        editing={reviewing.editing}
        blockers={reviewing.blockers}
        reviewed={reviewing.reviewed}
        write={reviewing.write}
        onSave={reviewing.save}
        onDrop={reviewing.drop}
        onMark={reviewing.mark}
        onApply={reviewing.apply}
        onDiscard={reviewing.discard}
      />
    </div>
  );
}

/** A journey proposal: the journey derived in the tab, unless the proposal creates it. */
function JourneyReview({ journey, ...props }: Omit<BodyProps, "ready" | "before"> & { journey: string }) {
  const creates = props.review.proposal.draft.destination_revision === 0;
  const view = useJourney(journey);
  if (view.status === "ready") {
    return <ReviewBody {...props} ready={view} before={view.journey.graph} />;
  }
  if (creates && view.status === "missing") {
    return <ReviewBody {...props} ready={undefined} before={undefined} />;
  }
  if (view.status === "failed") {
    return <p className="callout callout-bad">The journey could not be read: {view.message}</p>;
  }
  if (view.status === "skew") {
    return <p className="callout">This journey comes from a newer Cairn; reload to review the proposal.</p>;
  }
  return <p className="muted">Deriving the journey...</p>;
}

/** A route proposal: the route's draft as it is, or its latest version, or nothing for a new route. */
function RouteReview({ route, ...props }: Omit<BodyProps, "ready" | "before"> & { route: string }) {
  const { host } = useSession();
  const revision = props.review.stale?.conflict.current ?? props.review.proposal.draft.destination_revision;
  const [before, setBefore] = useState<{ graph: Graph | undefined } | undefined>();
  useEffect(() => {
    let live = true;
    const read = async (): Promise<Graph | undefined> => {
      const found = await host.route(route);
      if (found.draft != null) {
        return found.draft.graph;
      }
      const latest = Math.max(0, ...(found.versions ?? []));
      return latest === 0 ? undefined : (await host.routeVersion(route, latest)).graph;
    };
    read().then(
      (graph) => {
        if (live) {
          setBefore({ graph });
        }
      },
      () => {
        if (live) {
          setBefore({ graph: undefined });
        }
      },
    );
    return () => {
      live = false;
    };
  }, [host, route, revision]);
  if (before === undefined) {
    return <p className="muted">Reading the route...</p>;
  }
  return <ReviewBody {...props} ready={undefined} before={before.graph} />;
}

export function ProposalScreen() {
  const { id = "" } = useParams();
  const { view, refetch } = useLive(`proposal:${id}`, proposalReview(id));
  const deployment = useDeployment();
  switch (view.status) {
    case "loading":
      return <p className="muted">Reading the proposal...</p>;
    case "missing":
      return <p className="callout">This proposal does not exist. <Link to="/">All journeys</Link></p>;
    case "failed":
      return <p className="callout callout-bad">The proposal could not be read: {view.message}</p>;
    case "ready":
      break;
  }
  if (deployment === undefined) {
    return <p className="muted">Reading the deployment...</p>;
  }
  const review = view.value;
  const destination = review.proposal.destination;
  const props = { review, refetch, deployment };
  if (destination === "deployment") {
    return <ReviewBody key={id} {...props} ready={undefined} before={undefined} />;
  }
  return "journey" in destination ? <JourneyReview key={id} {...props} journey={destination.journey} /> : <RouteReview key={id} {...props} route={destination.route} />;
}
