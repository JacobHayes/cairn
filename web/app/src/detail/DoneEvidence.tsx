// C11, G2, G4: finishing work that requires a note or a link and has none. Done... opens the
// fields it needs; sending adds them and completes in one patch, so the guards see them. The
// inspector's Actions and the next list's rows both offer it, so a Complete that the engine
// would reject is never the primary action.
import { newAttachmentKey } from "./Attachments.tsx";
import { doneMutations, evidenceComplete, evidenceDraft, type Evidence, type EvidenceDraft } from "../acting/acts.ts";
import { Button, Field } from "../ui/kit.tsx";
import type { NodeWrite, useFormDraft } from "./write.ts";

/** What done needs added first. */
export interface Needs {
  artifact: boolean;
  note: boolean;
}

export type DoneForm = ReturnType<typeof useFormDraft<EvidenceDraft | string>>;

/** The words on the button that adds what done needs and completes. */
function sendLabel({ artifact, note }: Needs): string {
  if (artifact && note) {
    return "Add and mark done";
  }
  return artifact ? "Link and mark done" : "Add note and mark done";
}

/** The Done... button: opens the form once, keeping what was typed. */
export function DoneButton({ write, form }: { write: NodeWrite; form: DoneForm }) {
  return (
    <Button
      primary
      disabled={write.disabled}
      onClick={() => {
        if (form.draft === undefined) {
          form.open({ artifact: "", note: "" }, write.seen);
        }
      }}
    >
      Done...
    </Button>
  );
}

export function EvidenceForm({ write, node, needs, form }: { write: NodeWrite; node: string; needs: Needs; form: DoneForm }) {
  const drafted = form.draft;
  if (drafted === undefined) {
    return null;
  }
  const { artifact, note } = evidenceDraft(drafted.value);
  const send = async () => {
    const evidence: Evidence = {
      ...(needs.artifact ? { artifact: { key: newAttachmentKey(), url: artifact.trim() } } : {}),
      ...(needs.note ? { note: { key: newAttachmentKey(), text: note.trim() } } : {}),
    };
    if (await write.run(doneMutations(node, evidence), drafted)) {
      form.close();
    }
  };
  return (
    <div className="stack next-row-more" data-testid="done-evidence">
      {needs.artifact ? <Field autoFocus aria-label="Artifact address" placeholder="Its artifact's address" value={artifact} onChange={(event) => { form.change({ artifact: event.target.value, note }); }} /> : null}
      {needs.note ? <textarea autoFocus={!needs.artifact} aria-label="Note" placeholder="What was done, in a note" value={note} onChange={(event) => { form.change({ artifact, note: event.target.value }); }} /> : null}
      <span className="row">
        <Button primary disabled={write.disabled || !evidenceComplete(needs, { artifact, note })} onClick={() => void send()}>
          {sendLabel(needs)}
        </Button>
        <Button onClick={() => { form.close(); write.dismiss(); }}>Cancel</Button>
      </span>
    </div>
  );
}
