// E6, B3, H3: the deployment's entities. Each with its name, its emails (each held by one
// entity), and the old keys merged into it; made with a name alone and enriched later; edited;
// and merged, the merged key kept as an alias so journeys and their history resolve to the
// survivor unchanged. Which entities are the caller's is marked. Kept current (H6). An edit
// is drafted against the deployment revision its author saw, and forms survive a reload.
import { useState } from "react";
import { useLocation, useParams } from "react-router";

import type { Deployment } from "../data/host.ts";
import { useDraft } from "../data/drafts.ts";
import { useDeployment, useViewer } from "../data/react.ts";
import { Band } from "../proposals/Band.tsx";
import { DestinationProposals } from "../proposals/DestinationProposals.tsx";
import { DeploymentLayerHost } from "../proposals/Hosts.tsx";
import { entityDiff, useLayer, useReviewing, type EntityRow as DiffRow } from "../proposals/layer.ts";
import { DIFF_LABELS } from "../proposals/model.ts";
import { Rail } from "../proposals/Rail.tsx";
import type { Rejection } from "../data/writes.ts";
import { RejectionView } from "../screens/RejectionView.tsx";
import { WriteStatus } from "../screens/WriteStatus.tsx";
import { useScreenWrite, type ScreenWrite } from "../screens/write.ts";
import { NodePanel } from "../acting/NodeItem.tsx";
import { Chip, Chips } from "../ui/Chip.tsx";
import { ChipsInput } from "../ui/ChipsInput.tsx";
import { IndexHead } from "../ui/IndexHead.tsx";
import { Badge, Button, Field } from "../ui/kit.tsx";
import { MergeEntities } from "./MergeEntities.tsx";
import { aliasesOf, byName, EMAIL_RULES, entityOf, mergeAsked, newEntityKey, type Entity } from "./model.ts";
import { plural } from "../ui/words.ts";

/** An entity's edit as typed, and the deployment revision its author saw when they opened it (H5). */
interface EntityDraft {
  name: string;
  emails: string[];
  /** The email typed and not yet taken as a chip. */
  emailText: string;
  base: number;
}

/**
 * The deployment revision an author who keeps their edit after a conflict drafts it against:
 * the one the rejection reports (H5: only that was compared), or the one the tab holds if it
 * has moved further, so the retry works before the tick arrives.
 */
export function deploymentRebase(rejection: Rejection, held: number): number {
  if (rejection.rejection !== "stale") {
    return held;
  }
  const reported = rejection.conflicts.map((conflict) => ("domain" in conflict.of && conflict.of.domain === "deployment" ? conflict.current : 0));
  return Math.max(held, ...reported);
}

function EntityEditor({ entity, deployment, draft, setDraft, write }: { entity: Entity; deployment: Deployment; draft: EntityDraft; setDraft: (draft: EntityDraft | undefined) => void; write: ScreenWrite }) {
  const save = async () => {
    const mutation = { op: "edit_entity" as const, entity: entityOf(entity.key, draft.name, draft.emails) };
    if (await write.run({ target: "deployment", baseRevision: draft.base, mutations: [mutation] })) {
      setDraft(undefined);
    }
  };
  return (
    <>
      <form className="stack" onSubmit={(event) => { event.preventDefault(); void save(); }}>
        <span className="row">
          <Field label="Name" value={draft.name} onChange={(event) => { setDraft({ ...draft, name: event.target.value }); }} />
          <ChipsInput label="Emails" placeholder="Add an email" inputMode="email" rules={EMAIL_RULES} values={draft.emails} text={draft.emailText} onChange={(emails, emailText) => { setDraft({ ...draft, emails, emailText }); }} />
        </span>
        <span className="row">
          <Button primary type="submit" disabled={write.disabled || draft.name.trim() === "" || draft.emailText.trim() !== ""}>Save</Button>
          <Button onClick={() => { setDraft(undefined); write.dismiss(); }}>Cancel</Button>
        </span>
      </form>
      <WriteStatus view={write.status} />
      {write.rejection === undefined ? null : (
        <RejectionView
          rejection={write.rejection}
          onDismiss={write.dismiss}
          onRebase={() => {
            if (write.rejection !== undefined) {
              setDraft({ ...draft, base: deploymentRebase(write.rejection, deployment.revision) });
            }
            write.dismiss();
          }}
        />
      )}
    </>
  );
}

function EntityRow({ entity, deployment, yours, mark, intoName }: { entity: Entity; deployment: Deployment; yours: boolean; mark?: DiffRow["mark"]; intoName?: string | undefined }) {
  const reviewing = useReviewing() !== undefined;
  const [draft, setDraft] = useDraft<EntityDraft>(`entity-edit:${entity.key}`);
  // The row holds the write, so a save in flight keeps Edit closed until it settles: its
  // landing closes the draft it sent, never one opened after it.
  const write = useScreenWrite(`entity:${entity.key}`, entity.name);
  const aliases = aliasesOf(deployment, entity.key);
  return (
    <li className="entity-row" data-testid="entity" data-entity={entity.key} data-diff={mark?.kind}>
      <span className="entity-head">
        <span className="row entity-title">
          {mark === undefined ? null : (
            <span className="layer-word" data-testid="row-diff" data-diff={mark.kind}>
              {mark.into === undefined ? DIFF_LABELS[mark.kind] : `Merged into ${intoName ?? mark.into}`}
            </span>
          )}
          <strong data-testid="entity-name">{entity.name}</strong>
          {yours ? <Badge tone="good" data-testid="yours">You</Badge> : null}
          {aliases.length === 0 ? null : <span className="muted small" data-testid="aliases">merged from {plural(aliases.length, "other entry", "other entries")}</span>}
        </span>
        {draft === undefined && !reviewing ? (
          <Button disabled={write.disabled} onClick={() => { setDraft({ name: entity.name, emails: [...(entity.emails ?? [])], emailText: "", base: deployment.revision }); }}>Edit</Button>
        ) : null}
      </span>
      {draft !== undefined && !reviewing ? null : (
        <span data-testid="entity-emails">
          {(entity.emails ?? []).length === 0 ? <span className="muted small">No emails</span> : <Chips>{(entity.emails ?? []).map((email) => <Chip key={email}>{email}</Chip>)}</Chips>}
        </span>
      )}
      {draft === undefined || reviewing ? null : <EntityEditor entity={entity} deployment={deployment} draft={draft} setDraft={setDraft} write={write} />}
    </li>
  );
}

function CreateEntity({ deployment }: { deployment: Deployment }) {
  const write = useScreenWrite("new-entity");
  const [draft, setDraft] = useDraft<{ name: string; emails: string[]; emailText: string }>("entity-new");
  const { name, emails, emailText } = draft ?? { name: "", emails: [], emailText: "" };
  const create = async () => {
    const mutation = { op: "create_entity" as const, entity: entityOf(newEntityKey(name), name, emails) };
    if (await write.run({ target: "deployment", baseRevision: deployment.revision, mutations: [mutation] })) {
      setDraft(undefined);
    }
  };
  return (
    <form className="stack" data-testid="create-entity" onSubmit={(event) => { event.preventDefault(); void create(); }}>
      <span className="row">
        <Field label="Name" value={name} onChange={(event) => { setDraft({ name: event.target.value, emails, emailText }); }} />
        <ChipsInput label="Emails (optional)" placeholder="Add an email" inputMode="email" rules={EMAIL_RULES} values={emails} text={emailText} onChange={(next, nextText) => { setDraft({ name, emails: next, emailText: nextText }); }} />
      </span>
      <span className="row">
        <Button primary type="submit" disabled={write.disabled || name.trim() === "" || emailText.trim() !== ""}>Add an entity</Button>
        {write.rejection === undefined ? <WriteStatus view={write.status} /> : <RejectionView rejection={write.rejection} onDismiss={write.dismiss} />}
      </span>
    </form>
  );
}

/** The entities page: the entities as they are, or, with a proposal's layer on, as it would leave them with every change marked. */
function EntitiesList({ deployment, rows }: { deployment: Deployment; rows: DiffRow[] | undefined }) {
  const { viewer } = useViewer();
  const { search } = useLocation();
  const asked = mergeAsked(search);
  // Merging is closed until asked for, or until a link (the sign-in page's offer) names the two to merge.
  const [merge, setMerge] = useState<{ asked: string; open: boolean } | undefined>(undefined);
  const layer = useLayer();
  const reviewing = useReviewing() !== undefined;
  const mergeOpen = !reviewing && (merge?.asked === asked.join(",") ? merge.open : asked.length > 0);
  const yours = new Set(viewer?.entities ?? []);
  const listed: DiffRow[] = rows ?? byName(deployment.entities ?? []).map((entity) => ({ entity, mark: undefined }));
  const names = new Map(listed.map((row) => [row.entity.key, row.entity.name]));
  return (
    <section className="stack" aria-label="Entities" data-testid="entities" data-revision={deployment.revision} data-layer={layer?.mode} data-rows-page tabIndex={-1}>
      <IndexHead title="Entities">
        {reviewing ? null : <Button aria-expanded={mergeOpen} aria-controls="merge-entities" onClick={() => { setMerge({ asked: asked.join(","), open: !mergeOpen }); }}>{mergeOpen ? "Close merge" : "Merge entities"}</Button>}
      </IndexHead>
      <Band />
      <span className="muted small">People and teams journeys refer to, across every journey. They need not be users; an entity holding your verified email is you.</span>
      {reviewing ? null : <CreateEntity deployment={deployment} />}
      {mergeOpen ? <MergeEntities key={asked.join(",")} deployment={deployment} entities={byName(deployment.entities ?? [])} asked={asked} /> : null}
      {listed.length === 0 ? (
        <p className="muted small" data-testid="entities-empty">No entities yet.</p>
      ) : (
        <NodePanel title="Entities" count={listed.length} testId="entity-list">
          {listed.map(({ entity, mark }) => (
            <EntityRow key={entity.key} entity={entity} deployment={deployment} yours={yours.has(entity.key)} mark={mark} intoName={mark?.into === undefined ? undefined : names.get(mark.into)} />
          ))}
        </NodePanel>
      )}
    </section>
  );
}

/** Under the layer, the rows diffed (in the order the page lists them) and the rail beside them. */
function LayeredEntities() {
  const layer = useLayer();
  const deployment = layer?.deployment;
  if (layer === undefined || deployment === undefined) {
    return null;
  }
  const rows = layer.mode === "reviewing" ? entityDiff(deployment.before, deployment.after) : undefined;
  return (
    <>
      <EntitiesList deployment={deployment.before} rows={rows} />
      {layer.review === undefined ? null : <Rail selected={undefined} />}
    </>
  );
}

export function Entities() {
  const deployment = useDeployment();
  const proposal = useParams()["proposal"];
  if (deployment === undefined) {
    return <p className="muted small">Reading the entities...</p>;
  }
  return (
    <>
      {proposal === undefined ? <EntitiesList deployment={deployment} rows={undefined} /> : <DeploymentLayerHost proposal={proposal}>{() => <LayeredEntities />}</DeploymentLayerHost>}
      <DestinationProposals destination="deployment" />
    </>
  );
}
