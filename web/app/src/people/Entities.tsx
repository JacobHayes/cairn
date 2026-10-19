// E6, B3, H3: the deployment's entities. Each with its name, its emails (each held by one
// entity), and the old keys merged into it; made with a name alone and enriched later; edited;
// and merged, the merged key kept as an alias so journeys and their history resolve to the
// survivor unchanged. Which entities are the caller's is marked. Kept current (H6). An edit
// is drafted against the deployment revision its author saw, and forms survive a reload.
import { useState } from "react";
import { useLocation } from "react-router";

import type { Deployment } from "../data/host.ts";
import { useDraft } from "../data/drafts.ts";
import { valueOf } from "../data/keyed-read.ts";
import { useDeployment, useLive, useViewer } from "../data/react.ts";
import { RowsPanel } from "../acting/RowsPanel.tsx";
import { proposalRows } from "../proposals/ProposalRows.tsx";
import { listKey, proposalsUnder } from "../proposals/read.ts";
import type { Rejection } from "../data/writes.ts";
import { Refused } from "../screens/Refused.tsx";
import { WriteStatus } from "../screens/WriteStatus.tsx";
import { useScreenWrite, type ScreenWrite } from "../screens/write.ts";
import { NodePanel } from "../acting/NodeItem.tsx";
import { Chip, Chips } from "../ui/Chip.tsx";
import { ChipsInput } from "../ui/ChipsInput.tsx";
import { Badge, Button, Field } from "../ui/kit.tsx";
import { MergeEntities } from "./MergeEntities.tsx";
import { aliasesOf, byName, EMAIL_RULES, entityOf, mergeAsked, newEntityKey, type Entity } from "./model.ts";
import { Failed } from "../ui/Failed.tsx";

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
      {write.rejected === undefined ? null : (
        <Refused rejection={write.rejected} onDismiss={write.dismiss}>
          {write.rejected.rejection === "stale" ? (
            <Button onClick={() => { if (write.rejected !== undefined) { setDraft({ ...draft, base: deploymentRebase(write.rejected, deployment.revision) }); } write.dismiss(); }}>Keep my edit on the current version</Button>
          ) : null}
        </Refused>
      )}
    </>
  );
}

function EntityRow({ entity, deployment, yours }: { entity: Entity; deployment: Deployment; yours: boolean }) {
  const [draft, setDraft] = useDraft<EntityDraft>(`entity-edit:${entity.key}`);
  // The row holds the write, so a save in flight keeps Edit closed until it settles: its
  // landing closes the draft it sent, never one opened after it.
  const write = useScreenWrite(`entity:${entity.key}`, entity.name);
  const aliases = aliasesOf(deployment, entity.key);
  return (
    <li className="entity-row" data-testid="entity" data-entity={entity.key}>
      <span className="entity-head">
        <span className="row entity-title">
          <strong data-testid="entity-name">{entity.name}</strong>
          {yours ? <Badge tone="good" data-testid="yours">You</Badge> : null}
          {aliases.length === 0 ? null : <span className="muted small" data-testid="aliases">merged from {aliases.length} other {aliases.length === 1 ? "entry" : "entries"}</span>}
        </span>
        {draft === undefined ? (
          <Button disabled={write.disabled} onClick={() => { setDraft({ name: entity.name, emails: [...(entity.emails ?? [])], emailText: "", base: deployment.revision }); }}>Edit</Button>
        ) : null}
      </span>
      {draft !== undefined ? null : (
        <span data-testid="entity-emails">
          {(entity.emails ?? []).length === 0 ? <span className="muted small">No emails</span> : <Chips>{(entity.emails ?? []).map((email) => <Chip key={email}>{email}</Chip>)}</Chips>}
        </span>
      )}
      {draft === undefined ? null : <EntityEditor entity={entity} deployment={deployment} draft={draft} setDraft={setDraft} write={write} />}
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
        {write.rejected === undefined ? <WriteStatus view={write.status} /> : <Refused rejection={write.rejected} onDismiss={write.dismiss} />}
      </span>
    </form>
  );
}

/** C14: the open proposals for the deployment (an entity edited or merged by an agent), above the entities; nothing when none is waiting. */
function DeploymentProposals() {
  const { view, refresh } = useLive(listKey("deployment"), proposalsUnder("deployment"));
  const found = valueOf(view);
  if (view.status === "failed") {
    return (
      <p className="callout callout-bad" role="alert" data-testid="proposals-failed">
        <Failed what="The open proposals" error={view.error}>
          <Button onClick={refresh}>Try again</Button>
        </Failed>
      </p>
    );
  }
  if (found === undefined) {
    return <p className="muted small" data-testid="proposals-reading">Reading the open proposals...</p>;
  }
  return found.items.length === 0 ? null : <RowsPanel title="Open proposals" testId="deployment-proposals" rows={proposalRows(found.items)} />;
}

/** Merging is folded away until asked for, or until a link (the sign-in page's offer) names the two to merge. */
function MergeFold({ deployment, entities, asked }: { deployment: Deployment; entities: readonly Entity[]; asked: string[] }) {
  const [open, setOpen] = useState(asked.length > 0);
  return (
    <>
      <span className="row">
        <Button aria-expanded={open} aria-controls="merge-entities" onClick={() => { setOpen(!open); }}>{open ? "Close merge" : "Merge entities"}</Button>
      </span>
      {open ? <MergeEntities deployment={deployment} entities={entities} asked={asked} /> : null}
    </>
  );
}

export function Entities() {
  const deployment = useDeployment();
  const { viewer } = useViewer();
  const { search } = useLocation();
  const asked = mergeAsked(search);
  if (deployment === undefined) {
    return <p className="muted small">Reading the entities...</p>;
  }
  const yours = new Set(viewer?.entities ?? []);
  const entities = byName(deployment.entities ?? []);
  return (
    <section className="stack entities-page" aria-label="Entities" data-testid="entities" data-revision={deployment.revision} data-rows-page tabIndex={-1}>
      <h1>Entities</h1>
      <span className="muted small">People and teams journeys refer to, across every journey. They need not be users; an entity holding your verified email is you.</span>
      <DeploymentProposals />
      <CreateEntity deployment={deployment} />
      <MergeFold key={asked.join(",")} deployment={deployment} entities={entities} asked={asked} />
      {entities.length === 0 ? (
        <p className="muted small" data-testid="entities-empty">No entities yet.</p>
      ) : (
        <NodePanel title="Entities" count={entities.length} testId="entity-list">
          {entities.map((entity) => (
            <EntityRow key={entity.key} entity={entity} deployment={deployment} yours={yours.has(entity.key)} />
          ))}
        </NodePanel>
      )}
    </section>
  );
}
