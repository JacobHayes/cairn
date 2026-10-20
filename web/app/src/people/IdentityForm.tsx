// E6, B3, H3: the form that makes an identity (Add, at `/identities/new`) or changes one (Edit, in
// the identity's own inspector): a name and emails, either of which may be left out but not both.
// Both stand in the inspector, never on the page. A new identity is made with a random key; an
// edit is drafted against the deployment revision its author saw, and both drafts survive a reload.
import { useEffect, useState } from "react";
import { useNavigate } from "react-router";

import { pathTo } from "../address.ts";
import type { Deployment } from "../data/host.ts";
import { useDraft } from "../data/drafts.ts";
import type { Rejection } from "../data/writes.ts";
import { RejectionView } from "../screens/RejectionView.tsx";
import { WriteStatus } from "../screens/WriteStatus.tsx";
import { useScreenWrite, type ScreenWrite } from "../screens/write.ts";
import { ChipsInput } from "../ui/ChipsInput.tsx";
import { Button, Field } from "../ui/kit.tsx";
import { EMAIL_RULES, identityOf, nameProblem, newIdentityKey, type Identity } from "./model.ts";

/** What is typed in the form, and the email typed and not yet taken as a chip. */
interface Typed {
  name: string;
  emails: string[];
  emailText: string;
}

/** An identity's edit as typed, and the deployment revision its author saw when they opened it (H5). */
export interface IdentityDraft extends Typed {
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

/** Whether the typed identity may be sent: it names someone (a name, an email, or both), is within the limits, and holds no email still being typed. */
export function sendable(typed: Typed): boolean {
  return (typed.name.trim() !== "" || typed.emails.length > 0) && typed.emailText.trim() === "" && nameProblem(typed.name) === undefined;
}

/** The name and emails fields, stacked so their labels and boxes share an edge. */
function Fields({ typed, change }: { typed: Typed; change: (next: Typed) => void }) {
  const problem = nameProblem(typed.name);
  return (
    <>
      <Field label="Name" value={typed.name} onChange={(event) => { change({ ...typed, name: event.target.value }); }} />
      <ChipsInput label="Emails" placeholder="Add an email" inputMode="email" rules={EMAIL_RULES} values={typed.emails} text={typed.emailText} onChange={(emails, emailText) => { change({ ...typed, emails, emailText }); }} />
      {problem === undefined ? null : <span className="ink-bad" role="alert">{problem}</span>}
    </>
  );
}

/** What a form's write says under its buttons: its progress, or its rejection with the way on. */
function Outcome({ write, onRebase }: { write: ScreenWrite; onRebase?: (rejection: Rejection) => void }) {
  if (write.rejection === undefined) {
    return <WriteStatus view={write.status} />;
  }
  const { rejection } = write;
  return (
    <RejectionView
      rejection={rejection}
      onDismiss={write.dismiss}
      {...(onRebase === undefined ? {} : { onRebase: () => { onRebase(rejection); write.dismiss(); } })}
    />
  );
}

/** Add: a new identity, made with a random key, then opened in the inspector once the deployment holds it. */
export function NewIdentity({ deployment }: { deployment: Deployment }) {
  const navigate = useNavigate();
  const write = useScreenWrite("new-identity");
  const [draft, setDraft] = useDraft<Typed>("identity-new");
  const typed = draft ?? { name: "", emails: [], emailText: "" };
  const [made, setMade] = useState<string>();
  // The identity is opened when the deployment shows it, not when the write lands, so the inspector never looks for one that has not arrived.
  const arrived = made !== undefined && (deployment.identities ?? []).some((identity) => identity.key === made);
  useEffect(() => {
    if (made !== undefined && arrived) {
      void navigate(pathTo("identity", { key: made }), { replace: true });
    }
  }, [made, arrived, navigate]);
  const add = async () => {
    const key = newIdentityKey();
    if (await write.run({ target: "deployment", baseRevision: deployment.revision, mutations: [{ op: "create_identity", identity: identityOf(key, typed.name, typed.emails) }] })) {
      setDraft(undefined);
      setMade(key);
    }
  };
  return (
    <div className="detail-panel panel stack" data-testid="create-identity">
      <div className="stack detail-head">
        <span className="label">Identity</span>
        <h2>Add an identity</h2>
        <p className="muted">A person or team journeys can name. Give a name, an email, or both.</p>
      </div>
      <form className="stack" onSubmit={(event) => { event.preventDefault(); void add(); }}>
        <Fields typed={typed} change={setDraft} />
        <span className="row">
          <Button primary type="submit" disabled={write.disabled || made !== undefined || !sendable(typed)}>Add identity</Button>
          <Button onClick={() => { setDraft(undefined); write.dismiss(); void navigate(pathTo("identities")); }}>Cancel</Button>
        </span>
      </form>
      <Outcome write={write} />
    </div>
  );
}

/** Edit: the identity's name and emails, drafted against the deployment revision seen; the draft and the write are the panel's (so a save still out holds Edit shut), and Save or Cancel ends the draft. */
export function EditIdentity({ identity, deployment, draft, setDraft, write }: { identity: Identity; deployment: Deployment; draft: IdentityDraft; setDraft: (draft: IdentityDraft | undefined) => void; write: ScreenWrite }) {
  const save = async () => {
    if (await write.run({ target: "deployment", baseRevision: draft.base, mutations: [{ op: "edit_identity", identity: identityOf(identity.key, draft.name, draft.emails) }] })) {
      setDraft(undefined);
    }
  };
  return (
    <>
      <form className="stack" onSubmit={(event) => { event.preventDefault(); void save(); }}>
        <Fields typed={draft} change={(next) => { setDraft({ ...draft, ...next }); }} />
        <span className="row">
          <Button primary type="submit" disabled={write.disabled || !sendable(draft)}>Save</Button>
          <Button onClick={() => { setDraft(undefined); write.dismiss(); }}>Cancel</Button>
        </span>
      </form>
      <Outcome write={write} onRebase={(rejection) => { setDraft({ ...draft, base: deploymentRebase(rejection, deployment.revision) }); }} />
    </>
  );
}

/** The draft an Edit opens with: the identity as it is, against the deployment revision seen now. */
export function editOf(identity: Identity, deployment: Deployment): IdentityDraft {
  return { name: identity.name ?? "", emails: [...(identity.emails ?? [])], emailText: "", base: deployment.revision };
}
