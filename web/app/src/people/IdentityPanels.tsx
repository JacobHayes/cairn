// E6, H3: the Identities page's inspector. One identity (`/identities/<key>`) shows its name and
// emails, Edit, what refers to it (each journey and route a link) and Delete; several selected
// show who they are, Merge into one kept and Delete. Delete waits for the references read to
// answer and stays off while anything is named (references.ts); the reason is said in words.
import { IDENTITY_COUNT_PER_FILL_MAX } from "@cairn/client";
import { useActorRef, useSelector } from "@xstate/react";
import { useEffect, useRef, useState } from "react";
import { Link } from "react-router";

import { pathTo } from "../address.ts";
import { RowsPanel, type PanelRow } from "../acting/RowsPanel.tsx";
import type { Selection } from "../canvas/selection.ts";
import type { Deployment, IdentityReferences } from "../data/host.ts";
import { useDraft } from "../data/drafts.ts";
import { classify } from "../data/keyed-read.ts";
import { useEdit, useSession, useSkew } from "../data/react.ts";
import { landedWarning } from "../data/writes.ts";
import { journeyPath } from "../journeys/address.ts";
import { STATUS_WORDS } from "../journeys/StatusBadge.tsx";
import { routePath } from "../routes/address.ts";
import { RejectionView } from "../screens/RejectionView.tsx";
import { WriteStatus } from "../screens/WriteStatus.tsx";
import { useScreenWrite } from "../screens/write.ts";
import { Failed } from "../ui/Failed.tsx";
import { Badge, Button } from "../ui/kit.tsx";
import type { useReceipt } from "../ui/Receipt.tsx";
import { namedList, plural } from "../ui/words.ts";
import { EditIdentity, editOf, type IdentityDraft } from "./IdentityForm.tsx";
import { identityLabel } from "./label.ts";
import { mergeFlow } from "./merge.ts";
import { aliasesOf, byName, type Identity } from "./model.ts";
import { identityActions, referenceCount, routeFact, type Deletion, type Reading } from "./references.ts";

/** The emails an identity shows beside its name: an unnamed one is titled by its first email, so that one is not listed again. */
export function emailsShown(identity: Identity): string[] {
  const emails = identity.emails ?? [];
  return identity.name === undefined || identity.name === null ? emails.slice(1) : emails;
}

/** An identity's emails as plain mono text, one per line of the wrap. */
export function Emails({ identity }: { identity: Identity }) {
  const shown = emailsShown(identity);
  return shown.length === 0 ? null : (
    <span className="emails" data-testid="identity-emails">
      {shown.map((email) => (
        <span key={email} className="mono">{email}</span>
      ))}
    </span>
  );
}

/** Journeys and routes that name an identity, as rows of links. */
function referenceRows(references: IdentityReferences): PanelRow[] {
  return [
    ...references.journeys.map((journey) => ({
      key: `journey:${journey.id}`,
      render: () => (
        <li className="node-row" data-row data-testid="reference" data-kind="journey">
          <div className="node-row-main">
            <span className="node-row-text">
              <Link to={journeyPath({ journey: journey.id })} className="node-title" data-row-title>{journey.name}</Link>
              <span className="node-fact muted">Journey, {STATUS_WORDS[journey.status].toLowerCase()}</span>
            </span>
          </div>
        </li>
      ),
    })),
    ...references.routes.map((route) => ({
      key: `route:${route.header.id}`,
      render: () => (
        <li className="node-row" data-row data-testid="reference" data-kind="route">
          <div className="node-row-main">
            <span className="node-row-text">
              <Link to={routePath(route.header.id)} className="node-title" data-row-title>{route.header.name}</Link>
              <span className="node-fact muted">{routeFact(route)}</span>
            </span>
          </div>
        </li>
      ),
    })),
  ];
}

/** What refers to one identity, when anything does: a bounded panel of links. That nothing does, or that the read is going or failed, is Delete's to say. */
function References({ held }: { held: IdentityReferences | undefined }) {
  return held === undefined || referenceCount(held) === 0 ? null : <RowsPanel title="In these journeys and routes" testId="identity-references" rows={referenceRows(held)} />;
}

/** Why Delete is off, or what it will do, in words. `names` are the selected identities by their labels, `blocked` those something names. */
export function deleteWords(deletion: Deletion, names: readonly string[], blocked: readonly string[]): string {
  const them = namedList(names);
  const several = names.length > 1;
  switch (deletion.kind) {
    case "checking":
      return `Checking where ${them} ${several ? "are" : "is"} named...`;
    case "failed":
      return `Cairn could not check which journeys and routes name ${them}, so ${several ? "they" : "it"} cannot be deleted yet.`;
    case "referenced":
      return `${namedList(blocked)} ${blocked.length > 1 ? "are" : "is"} named in journeys or routes, so ${blocked.length > 1 ? "they" : "it"} cannot be deleted. ${several ? "Merge them into one instead." : "Select another identity as well to merge them instead."}`;
    case "allowed":
      return `Nothing names ${them}. Deleting removes ${several ? "them" : "it"} and ${several ? "their" : "its"} emails.`;
  }
}

/** Delete and why it is off. The references read that enables it can be a tick behind, so a click reads them again and deletes only if they still name nothing. */
function DeleteIdentities({ members, deletion, labels, refresh, onDeleted, deployment }: { members: readonly Identity[]; deletion: Deletion; labels: Record<string, string>; refresh: () => void; onDeleted: () => void; deployment: Deployment }) {
  const { host } = useSession();
  const write = useScreenWrite("delete-identities");
  const [found, setFound] = useState<Deletion>();
  // What the click found stands in for an allowed read; any other read already says why.
  const shown = found !== undefined && deletion.kind === "allowed" ? found : deletion;
  useEffect(() => {
    if (deletion.kind !== "allowed") {
      setFound(undefined);
    }
  }, [deletion.kind]);
  const names = members.map((member) => labels[member.key] ?? "");
  const blocked = shown.kind === "referenced" ? shown.identities.map((key) => labels[key] ?? "") : [];
  const [checking, setChecking] = useState(false);
  // The check and the write are one piece of work: a second click while it runs starts nothing.
  const remove = async () => {
    if (checking) {
      return;
    }
    setFound(undefined);
    setChecking(true);
    try {
      const reads = await Promise.all(members.map(async (member) => ({ key: member.key, references: await host.identityReferences(member.key) })));
      const named = reads.filter((read) => referenceCount(read.references) > 0).map((read) => read.key);
      if (named.length > 0) {
        setFound({ kind: "referenced", identities: named });
        refresh();
        return;
      }
      if (await write.run({ target: "deployment", baseRevision: deployment.revision, mutations: members.map((member) => ({ op: "delete_identity" as const, identity: member.key })) })) {
        onDeleted();
      }
    } catch (thrown) {
      const error = classify(thrown);
      setFound({ kind: "failed", error: error === "gone" ? { kind: "refused", message: "The identity is no longer there." } : error });
    } finally {
      setChecking(false);
    }
  };
  return (
    <div className="stack delete-group" data-testid="delete-identities" data-deletion={shown.kind}>
      {shown.kind === "failed" ? (
        <div className="callout callout-bad stack" role="alert" data-testid="delete-why">
          <p><Failed said={deleteWords(shown, names, blocked)} error={shown.error} retry /></p>
          <span className="row"><Button onClick={() => { setFound(undefined); refresh(); }}>Try again</Button></span>
        </div>
      ) : (
        <p className="muted" data-testid="delete-why">{deleteWords(shown, names, blocked)}</p>
      )}
      <span className="row">
        <Button disabled={shown.kind !== "allowed" || write.disabled || checking} onClick={() => void remove()}>
          {members.length === 1 ? "Delete" : `Delete ${String(members.length)}`}
        </Button>
      </span>
      {write.rejection === undefined ? <WriteStatus view={write.status} /> : <RejectionView rejection={write.rejection} onDismiss={write.dismiss} />}
    </div>
  );
}

/** One identity: who it is, Edit, what names it, Delete. */
export function OneIdentity({ identity, deployment, yours, reading, held, refresh, selection }: { identity: Identity; deployment: Deployment; yours: boolean; reading: Reading; held: IdentityReferences | undefined; refresh: () => void; selection: Selection }) {
  const [draft, setDraft] = useDraft<IdentityDraft>(`identity-edit:${identity.key}`);
  const label = identityLabel(identity);
  const write = useScreenWrite(`identity:${identity.key}`, label);
  const aliases = aliasesOf(deployment, identity.key);
  const actions = identityActions([identity.key], reading);
  return (
    <div className="detail-panel panel stack" data-testid="identity-detail" data-identity={identity.key}>
      <div className="stack detail-head">
        <div className="row detail-band">
          <span className="label">Identity</span>
          <span className="spacer" />
          {yours ? <Badge tone="good" data-testid="yours">You</Badge> : null}
        </div>
        <h2 data-testid="detail-title" className={identity.name === undefined || identity.name === null ? "mono" : undefined}>{label}</h2>
        {draft === undefined ? (
          <>
            <Emails identity={identity} />
            {aliases.length === 0 ? null : <span className="muted" data-testid="aliases">Merged from {plural(aliases.length, "other identity", "other identities")}</span>}
            <span className="row">
              <Button disabled={write.disabled} onClick={() => { setDraft(editOf(identity, deployment)); }}>Edit</Button>
            </span>
          </>
        ) : (
          <EditIdentity identity={identity} deployment={deployment} draft={draft} setDraft={setDraft} write={write} />
        )}
      </div>
      <References held={held} />
      <DeleteIdentities members={[identity]} deletion={actions.delete} labels={{ [identity.key]: label }} refresh={refresh} onDeleted={selection.clear} deployment={deployment} />
    </div>
  );
}

/** The identity kept by default: the caller's own among the members, else the first by name. */
export function keptDefault(members: readonly Identity[], yours: ReadonlySet<string>): string {
  return (members.find((member) => yours.has(member.key)) ?? byName(members)[0])?.key ?? "";
}

/** The one sentence of what a merge does. */
export function mergeWords(keptLabel: string, mergedLabels: readonly string[]): string {
  return `${namedList(mergedLabels)} will be merged into ${keptLabel}. Journeys that name ${mergedLabels.length === 1 ? "it" : "them"} will show ${keptLabel}.`;
}

/** The receipt a merge shows its warnings through: held above the panel, which a merge replaces. */
export type Receipted = ReturnType<typeof useReceipt>;

/** The merge flow and what follows its landing, held above the panels (a merge replaces the one that started it): its warning is shown and the survivor opened. */
export function useMerge(receipt: Receipted, selection: Selection) {
  const { host, edits, titleOf } = useSession();
  const flow = useActorRef(mergeFlow, { input: { edits, key: "merge-identities", references: (identity) => host.identityReferences(identity) } });
  // The survivor of the merge sent, which the choice on screen may since have moved off.
  const submitted = useRef<string | undefined>(undefined);
  useEffect(() => {
    const subscription = flow.on("merged", ({ answer }) => {
      receipt.show(landedWarning(answer, titleOf));
      if (submitted.current !== undefined) {
        // Opens the survivor alone, and lets the set go, even when the address is already on it.
        selection.pick(submitted.current);
      }
    });
    return () => {
      subscription.unsubscribe();
    };
  });
  return { flow, submitted };
}
export type Merging = ReturnType<typeof useMerge>;

/** Merge the members into the one kept. */
function MergeIdentities({ members, deployment, yours, receipt, merging }: { members: readonly Identity[]; deployment: Deployment; yours: ReadonlySet<string>; receipt: Receipted; merging: Merging }) {
  const { flow, submitted } = merging;
  const skew = useSkew();
  const working = useSelector(flow, (snapshot) => snapshot.hasTag("busy"));
  const failure = useSelector(flow, (snapshot) => snapshot.context.failure);
  const write = useEdit("merge-identities");
  const [chosen, setChosen] = useState<string>();
  const kept = members.some((member) => member.key === chosen) ? (chosen ?? "") : keptDefault(members, yours);
  const merged = members.filter((member) => member.key !== kept);
  const keptLabel = identityLabel(members.find((member) => member.key === kept) ?? {});
  return (
    <div className="stack" data-testid="merge-identities">
      <div className="stack">
        <span className="label" id="keep-label">Keep this identity</span>
        <div className="opts" role="radiogroup" aria-labelledby="keep-label">
          {members.map((member) => (
            <label key={member.key} className="opt">
              <input type="radio" name="keep" data-identity={member.key} checked={member.key === kept} disabled={working || write.busy} onChange={() => { setChosen(member.key); }} />
              {identityLabel(member)}
            </label>
          ))}
        </div>
      </div>
      <p data-testid="merge-sentence">{mergeWords(keptLabel, merged.map((member) => identityLabel(member)))}</p>
      <span className="row">
        <Button
          primary
          disabled={working || write.busy || skew}
          onClick={() => {
            receipt.clear();
            submitted.current = kept;
            flow.send({ type: "merge", survivor: kept, merged: merged.map((member) => member.key), deployment: deployment.revision });
          }}
        >
          Merge
        </Button>
      </span>
      {failure === undefined ? null : <p className="callout callout-bad" role="alert" data-testid="merge-failed"><Failed what="The merge" verb="made" error={failure} /></p>}
      {write.rejection === undefined ? (
        <WriteStatus view={write.status} />
      ) : (
        <RejectionView rejection={write.rejection} onDismiss={write.dismiss} />
      )}
    </div>
  );
}

/** Several identities selected: who they are, Merge and Delete over all of them. */
export function SeveralIdentities({ members, deployment, yours, reading, refresh, selection, receipt, merging }: { members: readonly Identity[]; deployment: Deployment; yours: ReadonlySet<string>; reading: Reading; refresh: () => void; selection: Selection; receipt: Receipted; merging: Merging }) {
  const labels = Object.fromEntries(members.map((member) => [member.key, identityLabel(member)]));
  const actions = identityActions(members.map((member) => member.key), reading);
  return (
    <div className="detail-panel panel stack" data-testid="selection-panel" data-count={members.length}>
      <div className="row">
        <strong>{members.length} selected</strong>
        <span className="spacer" />
        <Button ghost data-testid="selection-clear" onClick={selection.clear}>Clear</Button>
      </div>
      <ul className="plain-list node-rows selected-members">
        {members.map((member) => (
          <li key={member.key} className="node-row" data-testid="selection-member" data-identity={member.key}>
            <div className="node-row-main">
              <span className="node-row-text">
                <Link to={pathTo("identity", { key: member.key })} className="node-title" onClick={selection.release}>{identityLabel(member)}</Link>
                <Emails identity={member} />
              </span>
              <div className="row-side">
                <Button ghost aria-label={`Remove ${identityLabel(member)} from the selection`} title="Remove from the selection" onClick={() => { selection.toggle(member.key); }}>×</Button>
              </div>
            </div>
          </li>
        ))}
      </ul>
      {actions.merge.allowed ? (
        <MergeIdentities members={members} deployment={deployment} yours={yours} receipt={receipt} merging={merging} />
      ) : (
        <p className="muted" data-testid="merge-unavailable">{`Merging keeps one identity and takes at most ${String(IDENTITY_COUNT_PER_FILL_MAX)} more into it. Remove some from the selection.`}</p>
      )}
      <DeleteIdentities members={members} deletion={actions.delete} labels={labels} refresh={refresh} onDeleted={selection.clear} deployment={deployment} />
    </div>
  );
}

/** The inspector for an address that names an identity the deployment does not hold (deleted, or merged away). */
export function NoSuchIdentity() {
  return (
    <div className="detail-panel panel stack" data-testid="identity-gone">
      <p>That identity is not here any more. It may have been deleted, or merged into another.</p>
      <Link to={pathTo("identities")}>Back to the identities</Link>
    </div>
  );
}
