// C14: the review rail, in the inspector column while a proposal's layer is on: the one component
// every destination and every view shares. A fixed top (the proposal's head, its outline in one
// sentence, and what holds Apply or qualifies the preview), a body that is either the proposal's
// folds or the item editor of the node picked, and a foot with Discard. The holds and errors never
// live in the replaceable body, so a held Apply always has its line to focus whatever the body
// shows. Reading the proposal is here; acting on it is the band's Apply and this rail's editors.
import type { Schema } from "@cairn/client";
import { useMemo, type ReactNode } from "react";
import { Link, useNavigate } from "react-router";

import type { Kind, Role } from "../authoring/graph.ts";
import { treeOf } from "../authoring/graph.ts";
import { useDeployment, useUserWords } from "../data/react.ts";
import { Section } from "../detail/parts.tsx";
import { ShortfallView } from "../detail/ShortfallView.tsx";
import { useNodeAddresses } from "../journeys/address.ts";
import { Inspector } from "../shell/frame.tsx";
import { Badge, Button } from "../ui/kit.tsx";
import { Markdown } from "../ui/markdown.tsx";
import { Receipt } from "../ui/Receipt.tsx";
import { ChangeList } from "./Changes.tsx";
import { Problem } from "./Entries.tsx";
import { ItemList } from "./Items.tsx";
import { identityDiff, useLayer, type Layer, type LayerModel } from "./layer.ts";
import { DIFF_LABELS, itemNode, withMutationsAdded, type DiffKind, type DiffMark, type Proposal } from "./model.ts";
import { rolesAddedBy } from "./review.ts";
import { StalePanel } from "./Stale.tsx";
import { subjectOf, violationNode } from "../data/writes.ts";
import { fieldValueWords, itemHeading, mutationWords, outlineWords, stateValueWords } from "./words.ts";
import { fieldWords } from "../status/words.ts";
import { stateOf, type Blocker } from "./model.ts";
import "./proposals.css";

/** What each thing that stops an apply says, in the rail's "Apply waits for" line and on Apply's hover. */
export const BLOCKER_WORDS: Record<Blocker, string> = {
  not_open: "it is no longer open",
  editing: "an edit with a problem to be fixed or dropped",
  unsaved: "your unsaved edits",
  stale: "a refresh",
  unresolved: "the choices below",
  invalid: "the proposal to be valid",
  no_preview: "its preview",
};

/** "Apply waits for a refresh and the choices below": the holds in words, without counts; none when nothing holds. */
export function holdsWords(blockers: readonly Blocker[]): string | undefined {
  const words = blockers.filter((blocker) => blocker !== "not_open").map((blocker) => BLOCKER_WORDS[blocker]);
  if (words.length === 0) {
    return undefined;
  }
  const last = words.at(-1) ?? "";
  return `Apply waits for ${words.length === 1 ? last : `${words.slice(0, -1).join(", ")} and ${last}`}.`;
}

/** Before and after for the node picked: the fields that differ with what they were and are, or what it is to be added or removed. */
function BeforeAfter({ node, mark, model }: { node: string; mark: DiffMark | undefined; model: LayerModel }) {
  const { before, after, names } = model;
  if (after === undefined && before !== undefined) {
    return <p className="muted small">Not known until the proposal no longer breaks a rule; see the violations.</p>;
  }
  const was = (before?.nodes ?? []).find((each) => each.key === node);
  const now = (after?.nodes ?? []).find((each) => each.key === node);
  if (mark === undefined) {
    return <p className="muted small">Nothing about it changes.</p>;
  }
  if (was === undefined || now === undefined) {
    return <p className="muted small">{now === undefined ? "It is removed." : "It is new: nothing of it exists yet."}</p>;
  }
  const fields = Object.keys({ ...was, ...now }).filter((field) => field !== "key" && JSON.stringify((was as Record<string, unknown>)[field] ?? null) !== JSON.stringify((now as Record<string, unknown>)[field] ?? null));
  const [stateWas, stateNow] = [stateOf(before, node), stateOf(after, node)];
  const states = [...new Set([...Object.keys(stateWas), ...Object.keys(stateNow)])].filter((field) => JSON.stringify(stateWas[field] ?? null) !== JSON.stringify(stateNow[field] ?? null)).sort();
  if (fields.length === 0 && states.length === 0) {
    return <p className="muted small">What changes depends on the choice below.</p>;
  }
  return (
    <dl className="stack" data-testid="before-after">
      {fields.map((field) => (
        <div key={field} className="stack" data-testid="changed-field" data-field={field}>
          <dt className="muted small">{fieldWords(field)}</dt>
          <dd>
            <s>{fieldValueWords(field, (was as Record<string, unknown>)[field], names)}</s> {"→"} {fieldValueWords(field, (now as Record<string, unknown>)[field], names)}
          </dd>
        </div>
      ))}
      {states.map((field) => (
        <div key={field} className="stack" data-testid="changed-state" data-field={field}>
          <dt className="muted small">{STATE_WORDS[field] ?? field}</dt>
          <dd>
            <s>{stateValueWords(field, stateWas[field], node, names)}</s> {"→"} {stateValueWords(field, stateNow[field], node, names)}
          </dd>
        </div>
      ))}
    </dl>
  );
}

const STATE_WORDS: Record<string, string> = { nodes: "progress", answers: "answer", rationales: "reason", overrides: "override", pins: "pin", snoozes: "snooze", ticks: "ticked items" };

/** The change's word, in the tone its kind takes. */
function MarkWord({ mark }: { mark: DiffMark }) {
  return <span className={`layer-word layer-word-${mark.kind}`}>{DIFF_LABELS[mark.kind]}</span>;
}

/** A15, F5: the candidate's violations; a contradictory chain offers 5.1's resolution moves, each added to the proposal. */
function Violations({ layer, model }: { layer: Layer; model: LayerModel }) {
  const violations = model.preview.violations ?? [];
  if (violations.length === 0) {
    return null;
  }
  const chains = violations.flatMap((violation) => violation.chains?.chains ?? []);
  const before = layer.before;
  const reviewing = layer.reviewing;
  const move = model.editable && reviewing !== undefined ? (next: Schema<"Mutation">) => { reviewing.edit(withMutationsAdded(model.draft, [next])); } : undefined;
  return (
    <div className="callout callout-bad stack" role="alert" data-testid="proposal-violations">
      <strong>As it stands, applying it would be refused:</strong>
      <ul className="detail-list">
        {violations.map((violation, at) => {
          const node = violationNode(violation);
          return (
            <li key={at} data-testid="violation" data-code={violation.code} data-node={node}>
              {node === undefined ? null : <strong>{model.names.node(node)}: </strong>}
              {violation.message}
            </li>
          );
        })}
      </ul>
      {before === undefined ? null : chains.map((short, at) => <ShortfallView key={at} view={before} short={short} disabled={move === undefined} onMove={move} />)}
    </div>
  );
}

/** C14: a journey's frontier after, in rank order, what joins it marked, and what leaves it. */
function FrontierAfter({ layer, model }: { layer: Layer; model: LayerModel }) {
  const after = model.preview.frontier ?? [];
  const before = new Set(layer.before?.derived.frontier ?? []);
  const leaving = [...before].filter((key) => !after.includes(key));
  return (
    <div className="stack" aria-label="The frontier after" data-testid="frontier-after">
      {after.length === 0 ? <span className="muted">Nothing could be acted on after it.</span> : null}
      <ol className="detail-list">
        {after.map((key) => (
          <li key={key} data-testid="frontier-node" data-node={key} data-status={before.has(key) ? "kept" : "new"}>
            {model.names.node(key)} {before.has(key) || layer.before === undefined ? null : <Badge tone="good">new</Badge>}
          </li>
        ))}
      </ol>
      {leaving.length === 0 ? null : <span className="muted small" data-testid="frontier-leaving">No longer on it: {leaving.map((key) => model.names.node(key)).join(", ")}</span>}
    </div>
  );
}

/** What the rail says when the preview is not the whole story: the engine's refusal, edits not yet previewed, and a destination with no "after". */
function PreviewNotes({ layer, model }: { layer: Layer; model: LayerModel }) {
  const kept = layer.reviewing?.kept;
  return (
    <>
      {model.previewError === undefined ? null : <p className="callout callout-bad">No preview: {model.previewError}</p>}
      {kept !== undefined && !model.previewOfEdits ? (
        <p className="muted small" data-testid="preview-of-saved">
          {layer.kind === "journey" ? "Previewing your edits..." : "The preview shows the proposal as saved: save your edits to preview them."}
        </p>
      ) : null}
      {model.known || layer.kind === "deployment" || (model.preview.violations ?? []).length === 0 ? null : (
        <p className="muted small" data-testid="no-graph-after">As it stands it breaks a rule, so the views show the destination as it is, with the changes it names marked.</p>
      )}
    </>
  );
}

/** I6: the reviewer's edits not yet saved: kept, and saved or dropped from the rail, where they cannot be missed. */
function Unsaved({ layer }: { layer: Layer }) {
  const reviewing = layer.reviewing;
  const proposal = layer.review?.proposal;
  if (reviewing === undefined || reviewing.kept === undefined || proposal === undefined || proposal.status !== "open") {
    return null;
  }
  return (
    <span className="row callout" data-testid="unsaved">
      <span>Unsaved edits</span>
      {reviewing.kept.base === proposal.revision ? null : <span className="muted small">The proposal changed since you started editing.</span>}
      <Button primary disabled={reviewing.write.disabled || reviewing.editing} onClick={reviewing.save} data-testid="save-proposal">
        Save the edits
      </Button>
      <Button onClick={reviewing.drop}>Drop them</Button>
    </span>
  );
}

/** Who proposed it, and with which agent: where "Proposed by you" lives now (the rows say what it changes). */
function Origin({ proposal }: { proposal: Proposal }) {
  const userWords = useUserWords();
  return (
    <p className="meta muted" data-testid="proposal-origin">
      Proposed by {userWords(proposal.created_by)}
      {proposal.proposing_agent == null ? "" : " with an agent"}.
    </p>
  );
}

const NO_EDITORS: ReadonlySet<string> = new Set();

/** The contexts the item and change lists read, over the proposal as edited. */
function useContexts(layer: Layer, model: LayerModel) {
  const deployment = useDeployment();
  const structure = model.after ?? model.before ?? { nodes: [] };
  const tree = useMemo(() => treeOf(structure), [structure]);
  const roles: readonly Role[] = structure.roles ?? [];
  const kinds: readonly Kind[] = structure.participation_kinds ?? [];
  const reviewing = layer.reviewing;
  const placeholder = (model.draft.mutations ?? []).find((mutation) => mutation.op === "add_node")?.node.parent ?? undefined;
  const edit = reviewing?.edit ?? (() => undefined);
  const setInvalid = reviewing?.setInvalid ?? (() => undefined);
  const invalid = reviewing?.invalid ?? NO_EDITORS;
  return {
    items: { draft: model.draft, edit, names: model.names, roles, kinds, unresolved: model.unresolved, checking: model.checking, conflicts: model.conflicts, editable: model.editable },
    changes: {
      draft: model.draft,
      edit,
      names: model.names,
      tree,
      roles,
      deployment: deployment ?? { revision: 0, identities: [], aliases: {} },
      today: model.today,
      ready: layer.before,
      editable: model.editable,
      hasGraph: layer.kind !== "deployment",
      defaultParent: placeholder,
      invalid,
      setInvalid,
    },
  };
}

/** The key beside the Changes fold's head: each kind of change with a swatch of its fill and its count. */
function ChangeKey({ counts }: { counts: Record<DiffKind, number> }) {
  return (
    <span className="layer-key" data-testid="changes-key">
      {(["added", "changed", "removed"] as const).filter((kind) => counts[kind] > 0).map((kind) => (
        <span key={kind} className="layer-key-item" data-kind={kind}>
          <span className={`layer-swatch layer-swatch-${kind}`} aria-hidden="true" />
          {DIFF_LABELS[kind]} {counts[kind]}
        </span>
      ))}
    </span>
  );
}

/** The marked nodes, grouped by what the proposal does to them, then the requirements and the header and other changes in words. */
function ChangesFold({ model, onPick }: { model: LayerModel; onPick: (key: string) => void }) {
  const counts = { added: 0, changed: 0, removed: 0 };
  for (const mark of Object.values(model.marks)) {
    counts[mark.kind] += 1;
  }
  // A deployment proposal marks identities, not nodes; the key counts those the same way.
  const deployment = useLayer()?.deployment;
  for (const row of deployment === undefined ? [] : identityDiff(deployment.before, deployment.after)) {
    if (row.mark !== undefined) {
      counts[row.mark.kind] += 1;
    }
  }
  const { diff, entries, names } = model;
  const others = (model.draft.mutations ?? []).filter((mutation) => subjectOf(mutation) === undefined);
  const edges = [...diff.edgesAdded.map((edge) => ({ edge, kind: "added" as const })), ...diff.edgesRemoved.map((edge) => ({ edge, kind: "removed" as const }))];
  const total = entries.length + edges.length + others.length;
  return (
    <Section title="Changes" summary={<ChangeKey counts={counts} />} testId="changes-fold">
      {total === 0 ? <p className="muted">It changes nothing in the graph.</p> : null}
      <ul className="plain-list route-list" data-testid="diff-list">
        {entries.map((entry) => {
          const note = [entry.mark.note, entry.cause === undefined ? undefined : `with ${names.node(entry.cause)}`, entry.fields.length === 0 ? undefined : entry.fields.map(fieldWords).join(", ")].filter((part) => part !== undefined).join(" · ");
          return (
            <li key={entry.key} className="route-list-row" data-testid="diff-node" data-node={entry.key} data-status={entry.mark.kind} style={entry.cause === undefined ? undefined : { paddingLeft: "var(--space-6)" }}>
              <MarkWord mark={entry.mark} />
              <button type="button" className="link route-list-title" onClick={() => { onPick(entry.key); }}>
                {names.node(entry.key)}
              </button>
              {note === "" ? null : <span className="muted small">{note}</span>}
            </li>
          );
        })}
        {edges.map(({ edge, kind }) => (
          <li key={`${kind}:${edge.node}>${edge.requires}`} className="route-list-row" data-testid="diff-edge" data-status={kind}>
            <MarkWord mark={{ kind }} />
            <span className="route-list-title">
              {names.node(edge.node)} {kind === "added" ? "now requires" : "no longer requires"} {names.node(edge.requires)}
            </span>
          </li>
        ))}
        {others.map((mutation, at) => (
          <li key={at} className="route-list-row" data-testid="diff-other">
            <span className="route-list-title">{mutationWords(mutation, names)}</span>
          </li>
        ))}
      </ul>
    </Section>
  );
}

/** The fold of what still needs a choice: each item about a node picks it, the others are edited here. */
function ChoicesFold({ model, contexts, onPick }: { model: LayerModel; contexts: ReturnType<typeof useContexts>; onPick: (key: string) => void }) {
  const items = model.draft.items ?? [];
  const waiting = items.flatMap((item, index) => (contexts.items.unresolved.has(index) ? [{ item, index }] : []));
  const loose = (key: string | undefined) => key === undefined;
  const general = items.some((item) => loose(itemNode(item)));
  if (waiting.length === 0 && !general) {
    return null;
  }
  // The items about no node have no card to pick, so they stay here, resolved or not, to be read and changed.
  return (
    <Section title={waiting.length === 0 ? "Other choices" : "Needs a choice"} summary={waiting.length === 0 ? undefined : <Badge tone="warn">{waiting.length}</Badge>} open={waiting.length > 0} testId="to-decide">
      {waiting.length === 0 ? null : <ul className="plain-list route-list">
        {waiting.map(({ item, index }) => {
          const node = itemNode(item);
          return node === undefined ? null : (
            <li key={index} className="route-list-row" data-testid="to-decide-item">
              <button type="button" className="link route-list-title" onClick={() => { onPick(node); }}>
                {model.names.node(node)}
              </button>
              <span className="muted small">{itemHeading(item, model.names)}</span>
            </li>
          );
        })}
      </ul>}
      <ItemList context={contexts.items} itemNode={itemNode} only={loose} />
    </Section>
  );
}

/** C14: the node picked, as an item to edit: what changes in it, the choices it needs, and its own changes. */
function NodeEditor({ layer, model, node, back }: { layer: Layer; model: LayerModel; node: string; back: string }) {
  const contexts = useContexts(layer, model);
  const own = (key: string | undefined) => key === node;
  const items = (model.draft.items ?? []).some((item) => itemNode(item) === node);
  const mark = model.marks[node];
  const changes = (model.draft.mutations ?? []).some((mutation) => subjectOf(mutation) === node);
  return (
    <div className="stack item-editor" data-testid="item-editor" aria-label={model.names.node(node)}>
      <Link to={back} data-testid="back-to-proposal">
        {"‹"} The proposal
      </Link>
      <div className="stack detail-head">
        {mark === undefined ? null : (
          <span>
            <MarkWord mark={mark} />
          </span>
        )}
        <h2 data-testid="detail-title">{model.names.node(node)}</h2>
      </div>
      <Section title="What changes" open testId="item-before-after">
        <BeforeAfter node={node} mark={mark} model={model} />
      </Section>
      {items ? (
        <Section title="To decide" open testId="item-decide">
          <ItemList context={contexts.items} itemNode={itemNode} only={own} />
        </Section>
      ) : null}
      <Section title="Its changes" open={changes} testId="item-changes">
        <ChangeList context={contexts.changes} only={own} />
      </Section>
    </div>
  );
}

/** What the rail holds while no node is picked: the description, what still needs a choice, the changes, what can be acted on after, and every change. */
function ProposalBody({ layer, model, onPick }: { layer: Layer; model: LayerModel; onPick: (key: string) => void }) {
  const contexts = useContexts(layer, model);
  const proposal = layer.review?.proposal;
  const added = rolesAddedBy(model);
  return (
    <>
      {proposal?.draft.description == null ? null : <Markdown text={proposal.draft.description} />}
      {added.length === 0 ? null : (
        <p className="muted small" data-testid="roles-added">
          It adds {added.length === 1 ? "the role" : "the roles"} {added.map((role) => role.title ?? role.id).join(", ")}.
        </p>
      )}
      <div className="detail-folds">
        <ChoicesFold model={model} contexts={contexts} onPick={onPick} />
        <ChangesFold model={model} onPick={onPick} />
        {layer.kind !== "journey" || model.preview.frontier === undefined || !model.known ? null : (
          <Section title="What can be acted on after" testId="frontier-section">
            <FrontierAfter layer={layer} model={model} />
          </Section>
        )}
        <Section title="All its changes" testId="all-changes">
          <ChangeList context={contexts.changes} />
        </Section>
      </div>
    </>
  );
}

/** The head: the proposal's title, a chip only when it is not open, then `notes` (what holds Apply comes before the attribution, so a sheet at its peek says what is to be done), then who proposed it. */
function Head({ proposal, notes }: { proposal: Proposal; notes?: ReactNode }) {
  return (
    <header className="stack layer-head detail-head" data-testid="rail-head">
      <span className="row">
        <h2 data-testid="proposal-title">{proposal.draft.title}</h2>
        {proposal.status === "open" ? null : (
          <Badge tone={proposal.status === "applied" ? "good" : "warn"} data-testid="proposal-status" data-status={proposal.status}>
            {proposal.status}
          </Badge>
        )}
      </span>
      {notes}
      <Origin proposal={proposal} />
    </header>
  );
}

/** The rail's body for a proposal that is not under review: its title, proposer and changes in words, read only. */
function Closed({ layer }: { layer: Layer }) {
  const proposal = layer.review?.proposal;
  if (proposal === undefined) {
    return null;
  }
  return (
    <aside className="detail-panel panel stack rail" aria-label="The proposal" data-testid="proposal-rail" data-mode={layer.mode}>
      <Head proposal={proposal} />
      <p data-testid="rail-outline">{outlineWords(layer.review?.preview.outline ?? {})}</p>
      {proposal.draft.description == null ? null : <Markdown text={proposal.draft.description} />}
    </aside>
  );
}

/** C14: the review rail of the layer on the page; `selected` is the node picked, if any, whose item editor replaces the proposal's folds. */
export function Rail({ selected }: { selected: string | undefined }) {
  const layer = useLayer();
  const addresses = useNodeAddresses();
  const navigate = useNavigate();
  if (layer === undefined || layer.review === undefined) {
    return null;
  }
  const { model, reviewing, review } = layer;
  const proposal = review.proposal;
  if (layer.mode !== "reviewing" || model === undefined || reviewing === undefined) {
    return (
      <Inspector focus={`proposal:${layer.id}`} reveal={false}>
        <Closed layer={layer} />
      </Inspector>
    );
  }
  const onPick = (key: string) => void navigate(addresses.open(key), { state: { reveal: { key, how: "centre" } } });
  const holds = holdsWords(reviewing.blockers);
  const body: ReactNode =
    selected !== undefined && layer.kind !== "deployment" ? <NodeEditor layer={layer} model={model} node={selected} back={addresses.close()} /> : <ProposalBody layer={layer} model={model} onPick={onPick} />;
  return (
    <Inspector focus={`proposal:${layer.id}:${selected ?? ""}`} reveal={selected !== undefined}>
      <aside className="detail-panel panel stack rail" aria-label="The proposal" data-testid="proposal-rail" data-mode={layer.mode} data-revision={proposal.revision}>
        <div className="stack rail-lead">
          <Head
            proposal={proposal}
            notes={
              <div className="stack rail-notes" data-testid="rail-notes">
                {holds === undefined ? null : (
                  <p tabIndex={-1} className="rail-holds" role="status" data-testid="apply-waits" data-blockers={reviewing.blockers.join(" ")}>
                    {reviewing.blockers.includes("unresolved") ? (
                      <>
                        <span className="rail-holds-full">{holds}</span>
                        <span className="rail-holds-peek" aria-hidden="true">Choices needed. Tap to review.</span>
                      </>
                    ) : (
                      holds
                    )}
                  </p>
                )}
                <StalePanel review={review} disabled={!model.editable} onRefresh={reviewing.refresh} />
                <Problem write={reviewing.write} onRebase={reviewing.rebase} />
                <Receipt receipt={reviewing.write.receipt} />
                <PreviewNotes layer={layer} model={model} />
                <Unsaved layer={layer} />
                <Violations layer={layer} model={model} />
              </div>
            }
          />
          <p data-testid="rail-outline">{outlineWords(model.outline)}</p>
        </div>
        {body}
        {proposal.status !== "open" ? null : (
          <footer className="layer-foot">
            <Button disabled={reviewing.write.disabled} onClick={reviewing.discard} data-testid="discard-proposal">
              Discard the proposal
            </Button>
          </footer>
        )}
      </aside>
    </Inspector>
  );
}
