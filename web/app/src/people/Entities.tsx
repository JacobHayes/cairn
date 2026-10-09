// E6, B3, H3: the deployment's entities. Each with its name, its emails (each held by one
// entity), and the old keys merged into it; made with a name alone and enriched later; edited;
// and merged, the merged key kept as an alias so journeys and their history resolve to the
// survivor unchanged. Which entities are the caller's is marked. Kept current (H6). An edit
// is drafted against the deployment revision its author saw, and forms survive a reload.
import { useLocation } from "react-router";

import type { Deployment } from "../data/host.ts";
import { useDraft } from "../data/drafts.ts";
import { useDeployment, useViewer } from "../data/react.ts";
import type { Rejection } from "../data/writes.ts";
import { Refused } from "../screens/Refused.tsx";
import { useScreenWrite, type ScreenWrite } from "../screens/write.ts";
import { Badge, Button, Field } from "../ui/kit.tsx";
import { MergeEntities } from "./MergeEntities.tsx";
import { aliasesOf, byName, entityOf, newEntityKey, type Entity } from "./model.ts";

/** An entity's edit as typed, and the deployment revision its author saw when they opened it (H5). */
interface EntityDraft {
  name: string;
  emails: string;
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
      <form className="row" onSubmit={(event) => { event.preventDefault(); void save(); }}>
        <Field aria-label="Name" value={draft.name} onChange={(event) => { setDraft({ ...draft, name: event.target.value }); }} />
        <Field aria-label="Emails" placeholder="Emails, separated by commas" value={draft.emails} onChange={(event) => { setDraft({ ...draft, emails: event.target.value }); }} />
        <Button primary type="submit" disabled={write.disabled || draft.name.trim() === ""}>Save</Button>
        <Button onClick={() => { setDraft(undefined); write.dismiss(); }}>Cancel</Button>
      </form>
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
  const [draft, setDraft] = useDraft<EntityDraft>(`entity:${entity.key}`);
  // The row holds the write, so a save in flight keeps Edit closed until it settles: its
  // landing closes the draft it sent, never one opened after it.
  const write = useScreenWrite();
  const aliases = aliasesOf(deployment, entity.key);
  return (
    <li className="stack" data-testid="entity" data-entity={entity.key}>
      <span className="row">
        <strong data-testid="entity-name">{entity.name}</strong>
        <span className="muted mono">{entity.key}</span>
        {yours ? <Badge tone="good" data-testid="yours">You</Badge> : null}
        {aliases.length === 0 ? null : <span className="muted small" data-testid="aliases">also {aliases.join(", ")}</span>}
        <span className="spacer" />
        {draft === undefined ? (
          <Button disabled={write.disabled} onClick={() => { setDraft({ name: entity.name, emails: (entity.emails ?? []).join(", "), base: deployment.revision }); }}>Edit</Button>
        ) : null}
      </span>
      <span className="muted small" data-testid="entity-emails">{(entity.emails ?? []).join(", ") || "No emails"}</span>
      {draft === undefined ? null : <EntityEditor entity={entity} deployment={deployment} draft={draft} setDraft={setDraft} write={write} />}
    </li>
  );
}

function CreateEntity({ deployment }: { deployment: Deployment }) {
  const write = useScreenWrite();
  const [draft, setDraft] = useDraft<{ name: string; emails: string }>("entity:new");
  const { name, emails } = draft ?? { name: "", emails: "" };
  const create = async () => {
    const mutation = { op: "create_entity" as const, entity: entityOf(newEntityKey(name), name, emails) };
    if (await write.run({ target: "deployment", baseRevision: deployment.revision, mutations: [mutation] })) {
      setDraft(undefined);
    }
  };
  return (
    <form className="row" data-testid="create-entity" onSubmit={(event) => { event.preventDefault(); void create(); }}>
      <Field aria-label="New entity's name" placeholder="Name" value={name} onChange={(event) => { setDraft({ name: event.target.value, emails }); }} />
      <Field aria-label="New entity's emails" placeholder="Emails (optional)" value={emails} onChange={(event) => { setDraft({ name, emails: event.target.value }); }} />
      <Button primary type="submit" disabled={write.disabled || name.trim() === ""}>Add an entity</Button>
      {write.rejected === undefined ? null : <Refused rejection={write.rejected} onDismiss={write.dismiss} />}
    </form>
  );
}

export function Entities() {
  const deployment = useDeployment();
  const { viewer } = useViewer();
  const { search } = useLocation();
  const asked = (new URLSearchParams(search).get("merge") ?? "").split(",").filter((key) => key !== "");
  if (deployment === undefined) {
    return <p className="muted small">Reading the entities...</p>;
  }
  const yours = new Set(viewer?.entities ?? []);
  const entities = byName(deployment.entities ?? []);
  return (
    <section className="stack" aria-label="Entities" data-testid="entities" data-revision={deployment.revision}>
      <h1>Entities</h1>
      <span className="muted small">People and teams journeys refer to, across every journey. They need not be users; an entity holding your verified email is you.</span>
      <CreateEntity deployment={deployment} />
      <MergeEntities key={asked.join(",")} deployment={deployment} entities={entities} asked={asked} />
      <ul className="version-list">
        {entities.map((entity) => (
          <EntityRow key={entity.key} entity={entity} deployment={deployment} yours={yours.has(entity.key)} />
        ))}
      </ul>
    </section>
  );
}
