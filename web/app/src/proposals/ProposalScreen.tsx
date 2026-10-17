// C14: proposal review, for every destination (a journey, a route, the deployment, or one the
// proposal creates at revision 0), in the frame: the proposed graph as a diff (the Graph or the
// List) beside an inspector that is the item editor, edited item by item, previewed as edited,
// then saved, applied, or discarded (I6). A journey's preview is the engine's in the derive
// worker over the edited draft, with the frontier after (ARCHITECTURE, Web UI: Previews);
// any other destination's is the host's preview of the proposal as saved. A stale proposal
// shows what moved and is refreshed and reviewed again before it applies; apply sends the
// revision the reviewer confirmed reviewing, and is offered only when nothing blocks it.
import type { Schema } from "@cairn/client";
import { useCallback, useMemo, useState } from "react";
import { Link, useLocation, useNavigate, useParams } from "react-router";

import { destinationIn, proposalAddress, proposalsAddress, required } from "../address.ts";
import { treeOf, type Graph, type Kind, type Role } from "../authoring/graph.ts";
import { PREVIEW_SETTLE_MS } from "../authoring/write.ts";
import type { Deployment, Deriver, Host, RouteVersion } from "../data/host.ts";
import { beforeWork, HOST_READ, previewPolicy, valueOf, type ReadKey, type ReadSource } from "../data/keyed-read.ts";
import { useDraft } from "../data/drafts.ts";
import type { Answer } from "../data/edit-record.ts";
import type { ProposalReview } from "../data/proposals.ts";
import { useDeployment, useJourney, useKeyedRead, useLive, useSession, useViewer } from "../data/react.ts";
import { isPatchAnswer } from "../data/writes.ts";
import type { Ready } from "../detail/model.ts";
import { Section } from "../detail/parts.tsx";
import { ShortfallView } from "../detail/ShortfallView.tsx";
import { addedRoles, insertionMembers, noteOrigins } from "../segments/model.ts";
import { useSegmentNames } from "../segments/read.ts";
import { Inspector } from "../shell/frame.tsx";
import { useEscapeTo } from "../shell/useEscapeTo.ts";
import { Badge, Button, Panel } from "../ui/kit.tsx";
import { Markdown } from "../ui/markdown.tsx";
import { Receipt } from "../ui/Receipt.tsx";
import { ChangeList } from "./Changes.tsx";
import { Problem } from "./Entries.tsx";
import { ItemList } from "./Items.tsx";
import {
  applyBlockers,
  diffMarks,
  filterCounts,
  filterOf,
  graphDiff,
  itemNode,
  reviewedAfter,
  reviewEntries,
  unionGraph,
  unresolvedHere,
  withMutationsAdded,
  type DiffMark,
  type GraphDiff,
  type Proposal,
  type ProposalDraft,
  type ProposalPreview,
  type ReviewEntry,
  type ReviewFilter,
} from "./model.ts";
import { ProposalCanvas } from "./ProposalCanvas.tsx";
import { proposalReview, reviewKey } from "./read.ts";
import { BeforeAfter, BLOCKER_WORDS, FilterChips, ItemEditor, ProposalOrigin, ReviewHeader, ReviewList } from "./ReviewFrame.tsx";
import { StalePanel } from "./Stale.tsx";
import { itemHeading, mutationNode, namesOf, type Names } from "./words.ts";
import { useProposalWrite, type ProposalWrite } from "./write.ts";
import "./proposals.css";

/** The reviewer's edits not yet saved, and the proposal revision they started from (H5). */
interface Kept {
  base: number;
  draft: ProposalDraft;
}

const NOTHING_COMPARED: GraphDiff = { added: [], removed: [], changed: {}, edgesAdded: [], edgesRemoved: [] };

/** The route versions a journey proposal's mutations read: an upgrade's target and the journey's own, a re-link's new version, an insertion's segment. */
function versionsRead(ready: Ready, draft: ProposalDraft): Schema<"Lineage">[] {
  const own = ready.journey.header.lineage ?? undefined;
  return (draft.mutations ?? []).flatMap((mutation) => {
    if (mutation.op === "upgrade" && own !== undefined) {
      return [own, { route: own.route, version: mutation.to }];
    }
    if (mutation.op === "relink") {
      return own === undefined ? [mutation.lineage] : [own, mutation.lineage];
    }
    return mutation.op === "insert_segment" ? [mutation.segment] : [];
  });
}

type LocalPreview = { preview: ProposalPreview } | { error: string };

/** What the local preview is made from: the journey as derived, the proposal and its edits, and who previews. */
interface LocalArgs {
  host: Host;
  deriver: Deriver;
  ready: Ready;
  proposal: Proposal;
  draft: ProposalDraft;
  user: string;
}

/** C14: the edited proposal previewed by the engine over the journey the derive worker holds; the engine's refusal is the read's failure. */
const LOCAL_PREVIEW: ReadSource<LocalArgs, ProposalPreview> = {
  policy: previewPolicy(PREVIEW_SETTLE_MS),
  read: async ({ host, deriver, ready, proposal, draft, user }, at) => {
    const lineages = versionsRead(ready, draft);
    const versions: RouteVersion[] = await Promise.all(lineages.map((lineage) => host.routeVersion(lineage.route, lineage.version)));
    // B13: the engine reads a segment's header for its kind and retirement.
    const segments = await Promise.all([...new Set((draft.mutations ?? []).flatMap((mutation) => (mutation.op === "insert_segment" ? [mutation.segment.route] : [])))].map((route) => host.route(route)));
    // The edits may have moved on while the versions were out: obsolete work posts nothing.
    beforeWork(at);
    return deriver.preview(ready.journey.header.id, { proposal: { ...proposal, draft }, versions, segments, at: new Date().toISOString(), actor: { user } });
  },
};

/**
 * The key of the edited proposal's preview: the journey and the proposal are the identity; the
 * journey's derivation, the proposal's revision, the edits and who previews are what it is read from.
 */
function localPreviewKey(args: LocalArgs): ReadKey<LocalArgs> {
  return { identity: [args.ready.journey.header.id, args.proposal.id], freshness: [JSON.stringify(args.ready.key), args.proposal.revision, JSON.stringify(args.draft), args.user], args };
}

/** C14: the edited proposal previewed by the engine over the journey the derive worker holds, once the edits have held still. */
function useLocalPreview(ready: Ready | undefined, proposal: Proposal, draft: ProposalDraft): LocalPreview | undefined {
  const { deriver, host } = useSession();
  const { viewer } = useViewer();
  // Only an open proposal is previewed here: one applied or discarded has nothing left to apply.
  const previewed = proposal.status === "open" ? ready : undefined;
  const { view } = useKeyedRead(LOCAL_PREVIEW, previewed === undefined ? undefined : localPreviewKey({ host, deriver, ready: previewed, proposal, draft, user: viewer?.user ?? "u_local" }));
  if (view.status === "current") {
    return { preview: view.value };
  }
  return view.status === "failed" || view.status === "behind" ? { error: view.error.message } : undefined;
}

/**
 * Why there is no diff: the candidate breaks a rule, so there is no graph after; or the
 * destination is not a graph (the deployment's entities), so there is none to draw.
 */
function NoGraphAfter({ preview }: { preview: ProposalPreview | undefined }) {
  return (preview?.violations ?? []).length > 0 ? (
    <p className="muted small" data-testid="diff-unknown">Not known until it no longer breaks a rule; see the violations.</p>
  ) : (
    <p className="muted small" data-testid="diff-graphless">It changes the deployment's entities, not a graph; its changes are listed beside.</p>
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
      {after.length === 0 ? <span className="muted small">Nothing.</span> : null}
      <ol className="detail-list">
        {after.map((key) => (
          <li key={key} data-testid="frontier-node" data-node={key} data-status={before.has(key) ? "kept" : "new"}>
            {names.node(key)} {before.has(key) || ready === undefined ? null : <Badge tone="good">new</Badge>}
          </li>
        ))}
      </ol>
      {leaving.length === 0 ? null : <span className="muted small" data-testid="frontier-leaving">No longer on it: {leaving.map((key) => names.node(key)).join(", ")}</span>}
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

interface BodyProps {
  review: ProposalReview;
  refetch: () => void;
  /** The journey the proposal is for, derived, when it is one that exists. */
  ready: Ready | undefined;
  /** The destination's graph as it is (none for one the proposal creates). */
  before: Graph | undefined;
  /** The destination's name as it is now, when it has one. */
  name: string | undefined;
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
  const write = useProposalWrite(`proposal:${id}`, proposal.draft.title);
  const [kept, setKept] = useDraft<Kept>(`proposal:${id}`);
  const [reviewed, setReviewed] = useDraft<number>(`proposal-reviewed:${id}`);
  const { invalid, setInvalid } = useInvalidEditors();
  /** Reads the proposal again once a write landed, if the review is still there to read it. */
  const reread = () => {
    if (write.mounted()) {
      refetch();
    }
  };
  const saved = (answer: Answer | undefined, event: "saved" | "refreshed") => {
    if (answer === undefined || isPatchAnswer(answer)) {
      return;
    }
    if (answer.outcome !== "already_saved") {
      setReviewed(reviewedAfter(reviewed, { event, proposal: answer.proposal }));
    }
    reread();
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
        void write.run({ op: "edit", destination: proposal.destination, proposal: id, request: { base_revision: kept.base, draft: kept.draft } }).then((answer) => {
          if (answer !== undefined) {
            setKept(undefined);
          }
          saved(answer, "saved");
        });
      }
    },
    refresh: () => {
      void write.run({ op: "refresh", destination: proposal.destination, proposal: id, request: { base_revision: proposal.revision } }).then((answer) => {
        saved(answer, "refreshed");
      });
    },
    apply: () => {
      if (reviewed !== undefined) {
        void write.run({ op: "apply", destination: proposal.destination, proposal: id, request: { reviewed_revision: reviewed } }).then(reread);
      }
    },
    discard: () => {
      void write.run({ op: "discard", destination: proposal.destination, proposal: id, request: { base_revision: proposal.revision } }).then(reread);
    },
  };
}

/** What a review shows, worked out from the proposal as edited and the graphs before and after it. */
interface Model {
  draft: ProposalDraft;
  /** The graph after is known, so the diff and the frontier after can be shown. */
  known: boolean;
  preview: ProposalPreview;
  /** The preview is of the reviewer's edits (a journey's), or of the proposal as saved. */
  previewOfEdits: boolean;
  local: LocalPreview | undefined;
  before: Graph | undefined;
  after: Graph | undefined;
  diff: GraphDiff;
  marks: Record<string, DiffMark>;
  /** C19: the insertions it adds, named on their root cards and by member, and the roles it adds. */
  rolesAdded: ReturnType<typeof addedRoles>;
  entries: ReviewEntry[];
  counts: Record<ReviewFilter, number>;
  graphs: Graph[];
  names: Names;
  tree: ReturnType<typeof treeOf>;
  structure: Graph;
  today: string;
  editable: boolean;
}

function useReviewModel({ review, ready, before, deployment }: BodyProps, draft: ProposalDraft, writing: boolean): Model {
  const proposal = review.proposal;
  const local = useLocalPreview(ready, proposal, draft);
  // A journey's preview follows the edits; any other's is the host's, of the proposal as saved.
  const previewOfEdits = local !== undefined && "preview" in local;
  const preview = previewOfEdits ? local.preview : review.preview;
  const after = preview.graph ?? undefined;
  // A candidate that breaks a rule has no graph after; that is not an empty graph, so nothing is compared.
  const known = after !== undefined;
  const diff = useMemo(() => (known ? graphDiff(before, after) : NOTHING_COMPARED), [known, before, after]);
  const nameOf = useSegmentNames();
  const members = useMemo(() => insertionMembers(before, after, nameOf), [before, after, nameOf]);
  const marks = useMemo(() => noteOrigins(diffMarks(diff, draft.items ?? []), members), [diff, draft.items, members]);
  const added = useMemo<Graph>(() => ({ nodes: (draft.mutations ?? []).flatMap((mutation) => (mutation.op === "add_node" ? [mutation.node] : [])) }), [draft.mutations]);
  const structure = useMemo<Graph>(() => after ?? before ?? { nodes: [] }, [after, before]);
  return {
    draft,
    known,
    preview,
    previewOfEdits,
    local,
    before,
    after,
    diff,
    marks,
    rolesAdded: useMemo(() => addedRoles(before, after), [before, after]),
    entries: useMemo(() => reviewEntries(diff, marks, before), [diff, marks, before]),
    counts: useMemo(() => filterCounts(marks, diff), [marks, diff]),
    graphs: useMemo(() => (known ? [unionGraph(before, after), after, ...(before === undefined ? [] : [before])] : before === undefined ? [] : [before]), [known, before, after]),
    names: useMemo(() => namesOf([added, before, after], deployment.entities ?? []), [added, before, after, deployment]),
    tree: useMemo(() => treeOf(structure), [structure]),
    structure,
    today: ready?.key.today ?? new Date().toISOString().slice(0, 10),
    editable: proposal.status === "open" && !writing,
  };
}

/** What the head says when the preview is not the whole story: errors, edits not yet previewed, and a graph with no "after". */
function PreviewNotes({ model, kept, ready }: { model: Model; kept: Kept | undefined; ready: Ready | undefined }) {
  const { local, known, before, preview, previewOfEdits } = model;
  return (
    <>
      {local !== undefined && "error" in local ? <p className="callout callout-bad">No preview: {local.error}</p> : null}
      {kept !== undefined && !previewOfEdits ? <p className="muted small" data-testid="preview-of-saved">{ready === undefined ? "The preview shows the proposal as saved: save your edits to preview them." : "Previewing your edits..."}</p> : null}
      {known || before === undefined || (preview.violations ?? []).length === 0 ? null : <p className="muted small" data-testid="no-graph-after">As it stands it breaks a rule, so there is no graph after to compare: the canvas shows the graph as it is, with the items marked.</p>}
    </>
  );
}

/** I6: the reviewer's edits not yet saved: kept, and saved or dropped from the head, where they cannot be missed. */
function Unsaved({ proposal, kept, editing, write, onSave, onDrop }: { proposal: Proposal; kept: Kept | undefined; editing: boolean; write: ProposalWrite; onSave: () => void; onDrop: () => void }) {
  if (kept === undefined || proposal.status !== "open") {
    return null;
  }
  return (
    <span className="row callout" data-testid="unsaved">
      <strong>Your edits are not saved yet.</strong>
      {kept.base === proposal.revision ? null : <span className="muted small">The proposal changed since you started editing.</span>}
      <Button primary disabled={write.disabled || editing} onClick={onSave} data-testid="save-proposal">
        Save the edits
      </Button>
      <Button onClick={onDrop}>Drop them</Button>
    </span>
  );
}

type Reviewing = ReturnType<typeof useReviewing>;

/** I6: confirming the review, and what still stops Apply, at the foot of the inspector whatever it shows. */
function Confirm({ proposal, reviewing }: { proposal: Proposal; reviewing: Reviewing }) {
  if (proposal.status !== "open") {
    return null;
  }
  return (
    <section className="stack panel-actions" aria-label="Apply" data-testid="proposal-footer">
      <label className="row">
        <input type="checkbox" disabled={reviewing.kept !== undefined} checked={reviewing.reviewed === proposal.revision} onChange={(event) => { reviewing.mark(event.target.checked); }} data-testid="reviewed" />
        I have reviewed this proposal as it stands (revision {proposal.revision})
      </label>
      {reviewing.blockers.length === 0 ? null : (
        <ul className="detail-list muted small" data-testid="blockers">
          {reviewing.blockers.map((blocker) => (
            <li key={blocker} data-testid="blocker" data-blocker={blocker}>
              {BLOCKER_WORDS[blocker]}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

interface InspectorProps {
  proposal: Proposal;
  model: Model;
  reviewing: Reviewing;
  ready: Ready | undefined;
  name: string | undefined;
  deployment: Deployment;
  journey: string | undefined;
  selected: string | undefined;
  /** Opens a node's item editor. */
  onPick: (key: string) => void;
  /** Where "back to the proposal" goes. */
  back: string;
}

/** The contexts the item and change lists read, over the proposal as edited. */
function useContexts({ model, reviewing, ready, deployment, selected }: Pick<InspectorProps, "model" | "reviewing" | "ready" | "deployment" | "selected">) {
  const { draft, preview, names, tree, structure, today, editable, previewOfEdits } = model;
  const roles: readonly Role[] = structure.roles ?? [];
  const kinds: readonly Kind[] = structure.participation_kinds ?? [];
  const current = previewOfEdits || reviewing.kept === undefined;
  const unresolved = current ? new Map((preview.unresolved ?? []).map((item) => [item.item, item.reason])) : unresolvedHere(draft.items ?? []);
  const placeholder = (draft.mutations ?? []).find((mutation) => mutation.op === "add_node")?.node.parent ?? undefined;
  const { edit, setInvalid } = reviewing;
  return {
    items: { draft, edit, names, roles, kinds, unresolved, editable, selected },
    changes: { draft, edit, names, tree, roles, deployment, today, ready, editable, selected, defaultParent: placeholder, setInvalid },
  };
}

/** C14: the node picked, as an item to edit: what changes in it, the choices it needs, and its own changes. */
function NodeEditor(props: InspectorProps & { node: string }) {
  const { model, node, back } = props;
  const contexts = useContexts(props);
  const own = (key: string | undefined) => key === node;
  const items = (model.draft.items ?? []).some((item) => itemNode(item) === node);
  const mark = model.marks[node];
  const changes = (model.draft.mutations ?? []).some((mutation) => mutationNode(mutation) === node);
  return (
    <ItemEditor title={model.names.node(node)} mark={mark} back={back} footer={<Confirm proposal={props.proposal} reviewing={props.reviewing} />}>
      <Section title="What changes" open testId="item-before-after">
        <BeforeAfter node={node} mark={mark} before={model.before} after={model.after} names={model.names} />
      </Section>
      {items ? (
        <Section title="To decide" open testId="item-decide">
          <ItemList context={contexts.items} itemNode={itemNode} only={own} />
        </Section>
      ) : null}
      <Section title="Its changes" open={changes} testId="item-changes">
        <ChangeList context={contexts.changes} only={own} />
      </Section>
    </ItemEditor>
  );
}

/** What the inspector holds while no node is picked: the proposal's description, what still needs a choice, violations, what can be acted on after, and every change. */
function ProposalCard(props: InspectorProps) {
  const { proposal, model, ready, journey, onPick } = props;
  const contexts = useContexts(props);
  const { draft } = model;
  const waiting = (draft.items ?? []).flatMap((item, index) => (contexts.items.unresolved.has(index) ? [{ item, index }] : []));
  const loose = (key: string | undefined) => key === undefined;
  return (
    <aside className="detail-panel panel stack" aria-label="The proposal" data-testid="proposal-card">
      <h2>The proposal</h2>
      <ProposalOrigin proposal={proposal} name={props.name} />
      {proposal.draft.description == null ? null : <Markdown text={proposal.draft.description} />}
      {waiting.length === 0 ? null : (
        <section className="stack" aria-label="Still to decide" data-testid="to-decide">
          <span>
            <Badge tone="warn">Still to decide {waiting.length}</Badge>
          </span>
          <ul className="route-list">
            {waiting.map(({ item, index }) => {
              const node = itemNode(item);
              return (
                <li key={index} className="route-list-row" data-testid="to-decide-item">
                  <span className="route-list-title">
                    <strong>{node === undefined ? itemHeading(item, model.names) : model.names.node(node)}</strong> <span className="muted small">{node === undefined ? "" : itemHeading(item, model.names)}</span>
                  </span>
                  {node === undefined ? null : (
                    <Button onClick={() => { onPick(node); }}>Open</Button>
                  )}
                </li>
              );
            })}
          </ul>
        </section>
      )}
      <ItemList context={contexts.items} itemNode={itemNode} only={loose} />
      {model.rolesAdded.length === 0 ? null : (
        <p className="muted small" data-testid="roles-added">
          It adds {model.rolesAdded.length === 1 ? "the role" : "the roles"} {model.rolesAdded.map((role) => role.title ?? role.id).join(", ")}.
        </p>
      )}
      <Violations preview={model.preview} ready={ready} names={model.names} onMove={model.editable ? (move) => { contexts.changes.edit(withMutationsAdded(draft, [move])); } : undefined} />
      {model.preview.frontier === undefined || journey === undefined || !model.known ? null : (
        <Section title="What can be acted on after" testId="frontier-section">
          <FrontierAfter ready={ready} preview={model.preview} names={model.names} />
        </Section>
      )}
      <Section title="All its changes" testId="all-changes">
        <ChangeList context={contexts.changes} />
      </Section>
      <Confirm proposal={proposal} reviewing={props.reviewing} />
    </aside>
  );
}

/** The workspace: the diff as the Graph or the List, kept to the filter. */
function Workspace({ model, filter, listed, selected, domain, deployment, onPick }: { model: Model; filter: ReviewFilter; listed: boolean; selected: string | undefined; domain: string; deployment: Deployment; onPick: (key: string) => void }) {
  const kept = useMemo(() => (filter === "all" ? model.marks : Object.fromEntries(Object.entries(model.marks).filter(([, mark]) => filterOf(mark) === filter))), [model.marks, filter]);
  if (listed) {
    return model.known ? <ReviewList entries={model.entries} diff={model.diff} names={model.names} filter={filter} selected={selected} onPick={onPick} /> : <NoGraphAfter preview={model.preview} />;
  }
  return model.graphs.length === 0 ? <NoGraphAfter preview={model.preview} /> : <ProposalCanvas domain={domain} graphs={model.graphs} marks={kept} edges={model.diff} dim={filter !== "all"} deployment={deployment} today={model.today} selected={selected} onPick={onPick} />;
}

function ReviewBody(props: BodyProps) {
  const { review, ready } = props;
  const proposal = review.proposal;
  const reviewing = useReviewing(review, props.refetch);
  const { kept } = reviewing;
  const { key: selected } = useParams();
  const { search } = useLocation();
  const navigate = useNavigate();
  const [filter, setFilter] = useState<ReviewFilter>("all");
  const listed = new URLSearchParams(search).get("view") === "list";
  const model = useReviewModel(props, kept?.draft ?? proposal.draft, reviewing.write.disabled);
  const journey = proposal.destination !== "deployment" && "journey" in proposal.destination ? proposal.destination.journey : undefined;
  const base = proposalAddress(proposal.destination, proposal.id);
  useEscapeTo(selected === undefined ? undefined : `${base}${search}`);
  const onPick = (key: string) => void navigate(`${proposalAddress(proposal.destination, proposal.id, key)}${search}`);
  const inspector = { proposal, model, reviewing, ready, name: props.name, deployment: props.deployment, journey, selected, onPick, back: `${base}${search}` };
  const here = (view: "graph" | "list") => `${proposalAddress(proposal.destination, proposal.id, selected)}${view === "list" ? "?view=list" : ""}`;
  return (
    <>
      <div className="ws-fill journey-frame route-frame" data-testid="proposal" data-proposal={proposal.id} data-status={proposal.status} data-revision={proposal.revision}>
        <div className="ws-head stack journey-head">
          <ReviewHeader proposal={proposal} name={props.name} itemCount={model.counts.all} conflictCount={(model.draft.items ?? []).filter((item) => item.item === "conflict").length} blockers={reviewing.blockers} disabled={reviewing.write.disabled} onApply={reviewing.apply} onDiscard={reviewing.discard} />
          <StalePanel review={review} disabled={!model.editable} onRefresh={reviewing.refresh} />
          <Problem write={reviewing.write} onRebase={reviewing.rebase} />
          <Receipt receipt={reviewing.write.receipt} />
          <PreviewNotes model={model} kept={kept} ready={ready} />
          <Unsaved proposal={proposal} kept={kept} editing={reviewing.editing} write={reviewing.write} onSave={reviewing.save} onDrop={reviewing.drop} />
          <div className="row route-bar">
            <FilterChips filter={filter} counts={model.counts} onPick={setFilter} />
            <nav className="journey-switcher" aria-label="Projection" data-testid="projection-switcher">
              {(["graph", "list"] as const).map((each) => (
                <Link key={each} className="journey-switch" aria-current={(each === "list") === listed ? "page" : undefined} data-testid={`projection-${each}`} to={here(each)}>
                  {each === "graph" ? "Graph" : "List"}
                </Link>
              ))}
            </nav>
          </div>
        </div>
        <div className={listed ? "journey-body journey-scroll stack" : "ws-canvas journey-body"}>
          <Workspace model={model} filter={filter} listed={listed} selected={selected} domain={proposal.id} deployment={props.deployment} onPick={onPick} />
        </div>
      </div>
      <Inspector focus={`${proposal.id}:${selected ?? ""}`} reveal={selected !== undefined}>
        <div className="stack">
          {selected === undefined ? <ProposalCard {...inspector} /> : <NodeEditor {...inspector} node={selected} />}
        </div>
      </Inspector>
    </>
  );
}

/** A journey proposal: the journey derived in the tab, unless the proposal creates it. */
function JourneyReview({ journey, ...props }: Omit<BodyProps, "ready" | "before" | "name"> & { journey: string }) {
  const creates = props.review.proposal.draft.destination_revision === 0;
  const view = useJourney(journey);
  if (view.status === "ready") {
    return <ReviewBody {...props} ready={view} before={view.journey.graph} name={view.journey.header.name} />;
  }
  if (creates && view.status === "missing") {
    return <ReviewBody {...props} ready={undefined} before={undefined} name={undefined} />;
  }
  if (view.status === "failed") {
    return <p className="callout callout-bad">The journey could not be read: {view.message}</p>;
  }
  if (view.status === "skew") {
    return <p className="callout">This journey comes from a newer Cairn; reload to review the proposal.</p>;
  }
  return <p className="muted small">Deriving the journey...</p>;
}

/** The graph a route proposal is reviewed against: the route's draft as it is, or its latest version; none for a route that does not exist yet. */
const ROUTE_BEFORE: ReadSource<{ host: Host; route: string }, { graph: Graph | undefined; name: string }> = {
  policy: HOST_READ,
  read: async ({ host, route }) => {
    const found = await host.route(route);
    const name = found.header.name;
    if (found.draft != null) {
      return { graph: found.draft.graph, name };
    }
    const latest = Math.max(0, ...(found.versions ?? []));
    return { graph: latest === 0 ? undefined : (await host.routeVersion(route, latest)).graph, name };
  },
};

/** A route proposal: the route's draft as it is, or its latest version, or nothing for a new route. */
function RouteReview({ route, ...props }: Omit<BodyProps, "ready" | "before" | "name"> & { route: string }) {
  const { host } = useSession();
  const revision = props.review.stale?.conflict.current ?? props.review.proposal.draft.destination_revision;
  const { view } = useKeyedRead(ROUTE_BEFORE, { identity: [route], freshness: [revision], args: { host, route } });
  // A route that is not there yet is reviewed against nothing; one that cannot be read is not reviewed at all.
  const before = valueOf(view) ?? (view.status === "gone" ? { graph: undefined, name: undefined } : undefined);
  if (view.status === "failed") {
    return <p className="callout callout-bad" role="alert">The route could not be read: {view.error.message}</p>;
  }
  if (before === undefined) {
    return <p className="muted small">Reading the route...</p>;
  }
  return <ReviewBody {...props} ready={undefined} before={before.graph} name={before.name} />;
}

export function ProposalScreen() {
  const params = useParams();
  const id = required(params, "proposal");
  const destination = destinationIn(params);
  const { view, refresh: refetch } = useLive(reviewKey(destination, id), proposalReview(destination, id));
  const deployment = useDeployment();
  const review = valueOf(view);
  if (review === undefined) {
    if (view.status === "gone") {
      return (
        <p className="callout" data-testid="no-such-proposal">
          There is no such proposal here. <Link to={proposalsAddress(destination)}>Open proposals</Link>
        </p>
      );
    }
    return view.status === "failed" ? <p className="callout callout-bad">The proposal could not be read: {view.error.message}</p> : <p className="muted small">Reading the proposal...</p>;
  }
  if (deployment === undefined) {
    return <p className="muted small">Reading the deployment...</p>;
  }
  const props = { review, refetch, deployment };
  if (destination === "deployment") {
    return <ReviewBody key={id} {...props} ready={undefined} before={undefined} name={undefined} />;
  }
  return "journey" in destination ? <JourneyReview key={id} {...props} journey={destination.journey} /> : <RouteReview key={id} {...props} route={destination.route} />;
}
