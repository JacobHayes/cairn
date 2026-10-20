// E6, B3, H3: the deployment's identities. Each with its name, its emails (each held by one
// identity), and the old keys merged into it; made with a name alone and enriched later; edited;
// and merged, the merged key kept as an alias so journeys and their history resolve to the
// survivor unchanged. Which identities are the caller's is marked. Kept current (H6). An edit
// is drafted against the deployment revision its author saw, and forms survive a reload.
import { useState } from "react";
import { useLocation, useParams } from "react-router";

import type { Deployment } from "../data/host.ts";
import { useDraft } from "../data/drafts.ts";
import { useDeployment, useViewer } from "../data/react.ts";
import { Band } from "../proposals/Band.tsx";
import { DestinationProposals } from "../proposals/DestinationProposals.tsx";
import { DeploymentLayerHost } from "../proposals/Hosts.tsx";
import { identityDiff, useLayer, useReviewing, type IdentityRow as DiffRow } from "../proposals/layer.ts";
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
import { MergeIdentities } from "./MergeIdentities.tsx";
import { aliasesOf, byName, EMAIL_RULES, identityOf, mergeAsked, newIdentityKey, type Identity } from "./model.ts";
import { plural } from "../ui/words.ts";

/** An identity's edit as typed, and the deployment revision its author saw when they opened it (H5). */
interface IdentityDraft {
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

function IdentityEditor({ identity, deployment, draft, setDraft, write }: { identity: Identity; deployment: Deployment; draft: IdentityDraft; setDraft: (draft: IdentityDraft | undefined) => void; write: ScreenWrite }) {
  const save = async () => {
    const mutation = { op: "edit_identity" as const, identity: identityOf(identity.key, draft.name, draft.emails) };
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

function IdentityRow({ identity, deployment, yours, mark, intoName }: { identity: Identity; deployment: Deployment; yours: boolean; mark?: DiffRow["mark"]; intoName?: string | undefined }) {
  const reviewing = useReviewing() !== undefined;
  const [draft, setDraft] = useDraft<IdentityDraft>(`identity-edit:${identity.key}`);
  // The row holds the write, so a save in flight keeps Edit closed until it settles: its
  // landing closes the draft it sent, never one opened after it.
  const write = useScreenWrite(`identity:${identity.key}`, identity.name);
  const aliases = aliasesOf(deployment, identity.key);
  return (
    <li className="identity-row" data-testid="identity" data-identity={identity.key} data-diff={mark?.kind}>
      <span className="identity-head">
        <span className="row identity-title">
          {mark === undefined ? null : (
            <span className="layer-word" data-testid="row-diff" data-diff={mark.kind}>
              {mark.into === undefined ? DIFF_LABELS[mark.kind] : `Merged into ${intoName ?? mark.into}`}
            </span>
          )}
          <strong data-testid="identity-name">{identity.name}</strong>
          {yours ? <Badge tone="good" data-testid="yours">You</Badge> : null}
          {aliases.length === 0 ? null : <span className="muted small" data-testid="aliases">merged from {plural(aliases.length, "other entry", "other entries")}</span>}
        </span>
        {draft === undefined && !reviewing ? (
          <Button disabled={write.disabled} onClick={() => { setDraft({ name: identity.name, emails: [...(identity.emails ?? [])], emailText: "", base: deployment.revision }); }}>Edit</Button>
        ) : null}
      </span>
      {draft !== undefined && !reviewing ? null : (
        <span data-testid="identity-emails">
          {(identity.emails ?? []).length === 0 ? <span className="muted small">No emails</span> : <Chips>{(identity.emails ?? []).map((email) => <Chip key={email}>{email}</Chip>)}</Chips>}
        </span>
      )}
      {draft === undefined || reviewing ? null : <IdentityEditor identity={identity} deployment={deployment} draft={draft} setDraft={setDraft} write={write} />}
    </li>
  );
}

function CreateIdentity({ deployment }: { deployment: Deployment }) {
  const write = useScreenWrite("new-identity");
  const [draft, setDraft] = useDraft<{ name: string; emails: string[]; emailText: string }>("identity-new");
  const { name, emails, emailText } = draft ?? { name: "", emails: [], emailText: "" };
  const create = async () => {
    const mutation = { op: "create_identity" as const, identity: identityOf(newIdentityKey(), name, emails) };
    if (await write.run({ target: "deployment", baseRevision: deployment.revision, mutations: [mutation] })) {
      setDraft(undefined);
    }
  };
  return (
    <form className="stack" data-testid="create-identity" onSubmit={(event) => { event.preventDefault(); void create(); }}>
      <span className="row">
        <Field label="Name" value={name} onChange={(event) => { setDraft({ name: event.target.value, emails, emailText }); }} />
        <ChipsInput label="Emails (optional)" placeholder="Add an email" inputMode="email" rules={EMAIL_RULES} values={emails} text={emailText} onChange={(next, nextText) => { setDraft({ name, emails: next, emailText: nextText }); }} />
      </span>
      <span className="row">
        <Button primary type="submit" disabled={write.disabled || name.trim() === "" || emailText.trim() !== ""}>Add an identity</Button>
        {write.rejection === undefined ? <WriteStatus view={write.status} /> : <RejectionView rejection={write.rejection} onDismiss={write.dismiss} />}
      </span>
    </form>
  );
}

/** The identities page: the identities as they are, or, with a proposal's layer on, as it would leave them with every change marked. */
function IdentitiesList({ deployment, rows }: { deployment: Deployment; rows: DiffRow[] | undefined }) {
  const { viewer } = useViewer();
  const { search } = useLocation();
  const asked = mergeAsked(search);
  // Merging is closed until asked for, or until a link (the sign-in page's offer) names the two to merge.
  const [merge, setMerge] = useState<{ asked: string; open: boolean } | undefined>(undefined);
  const layer = useLayer();
  const reviewing = useReviewing() !== undefined;
  const mergeOpen = !reviewing && (merge?.asked === asked.join(",") ? merge.open : asked.length > 0);
  const yours = new Set(viewer?.identities ?? []);
  const listed: DiffRow[] = rows ?? byName(deployment.identities ?? []).map((identity) => ({ identity, mark: undefined }));
  const names = new Map(listed.map((row) => [row.identity.key, row.identity.name]));
  return (
    <section className="stack" aria-label="Identities" data-testid="identities" data-revision={deployment.revision} data-layer={layer?.mode} data-rows-page tabIndex={-1}>
      <IndexHead title="Identities">
        {reviewing ? null : <Button aria-expanded={mergeOpen} aria-controls="merge-identities" onClick={() => { setMerge({ asked: asked.join(","), open: !mergeOpen }); }}>{mergeOpen ? "Close merge" : "Merge identities"}</Button>}
      </IndexHead>
      <Band />
      <span className="muted small">People and teams journeys refer to, across every journey. They need not be users; an identity holding your verified email is you.</span>
      {reviewing ? null : <CreateIdentity deployment={deployment} />}
      {mergeOpen ? <MergeIdentities key={asked.join(",")} deployment={deployment} identities={byName(deployment.identities ?? [])} asked={asked} /> : null}
      {listed.length === 0 ? (
        <p className="muted small" data-testid="identities-empty">No identities yet.</p>
      ) : (
        <NodePanel title="Identities" count={listed.length} testId="identity-list">
          {listed.map(({ identity, mark }) => (
            <IdentityRow key={identity.key} identity={identity} deployment={deployment} yours={yours.has(identity.key)} mark={mark} intoName={mark?.into === undefined ? undefined : names.get(mark.into)} />
          ))}
        </NodePanel>
      )}
    </section>
  );
}

/** Under the layer, the rows diffed (in the order the page lists them) and the rail beside them. */
function LayeredIdentities() {
  const layer = useLayer();
  const deployment = layer?.deployment;
  if (layer === undefined || deployment === undefined) {
    return null;
  }
  const rows = layer.mode === "reviewing" ? identityDiff(deployment.before, deployment.after) : undefined;
  return (
    <>
      <IdentitiesList deployment={deployment.before} rows={rows} />
      {layer.review === undefined ? null : <Rail selected={undefined} />}
    </>
  );
}

export function Identities() {
  const deployment = useDeployment();
  const proposal = useParams()["proposal"];
  if (deployment === undefined) {
    return <p className="muted small">Reading the identities...</p>;
  }
  return (
    <>
      {proposal === undefined ? <IdentitiesList deployment={deployment} rows={undefined} /> : <DeploymentLayerHost proposal={proposal}>{() => <LayeredIdentities />}</DeploymentLayerHost>}
      <DestinationProposals destination="deployment" />
    </>
  );
}
