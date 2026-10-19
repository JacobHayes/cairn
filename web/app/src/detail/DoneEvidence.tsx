// C11, G2, G4: finishing work that requires a note or a link and has none. Mark done... opens the
// fields it needs; sending adds them and completes in one patch, so the guards see them. The
// inspector's Actions and the next list's rows both offer it, so a Complete that the engine
// would reject is never the primary action.
import { newAttachmentKey } from "./Attachments.tsx";
import { evidenceComplete, type Evidence, type EvidenceDraft } from "../acting/acts.ts";
import { Button, Field } from "../ui/kit.tsx";
import { Saving, type Sending } from "../ui/Receipt.tsx";

/** What done needs added first. */
export interface Needs {
  artifact: boolean;
  note: boolean;
}

/** The words on the button that adds what done needs and completes. */
function sendLabel({ artifact, note }: Needs): string {
  if (artifact && note) {
    return "Add and mark done";
  }
  return artifact ? "Link and mark done" : "Add note and mark done";
}

/** What sending `draft` adds before completing: each field the node needs, trimmed, with its new key. */
export function evidenceOf(needs: Needs, draft: EvidenceDraft): Evidence {
  return {
    ...(needs.artifact ? { artifact: { key: newAttachmentKey(), url: draft.artifact.trim() } } : {}),
    ...(needs.note ? { note: { key: newAttachmentKey(), text: draft.note.trim() } } : {}),
  };
}

/**
 * The fields done needs and the button that sends them. The form holds no state and sends
 * nothing itself: its owner keeps the draft, submits the write, and says when one is out
 * (`sending`: Cancel reads Close, because closing leaves the save going).
 */
export function EvidenceForm({
  needs,
  value,
  sending,
  disabled,
  onChange,
  onSend,
  onClose,
}: {
  needs: Needs;
  value: EvidenceDraft;
  sending: Sending;
  disabled: boolean;
  onChange: (next: EvidenceDraft) => void;
  onSend: (evidence: Evidence) => void;
  onClose: () => void;
}) {
  const { artifact, note } = value;
  return (
    <div className="stack row-more" data-testid="done-evidence">
      {needs.artifact ? <Field autoFocus aria-label="Artifact address" placeholder="Its artifact's address" value={artifact} onChange={(event) => { onChange({ artifact: event.target.value, note }); }} /> : null}
      {needs.note ? <textarea autoFocus={!needs.artifact} aria-label="Note" placeholder="What was done, in a note" value={note} onChange={(event) => { onChange({ artifact, note: event.target.value }); }} /> : null}
      <span className="row">
        <Button primary disabled={disabled || !evidenceComplete(needs, { artifact, note })} onClick={() => { onSend(evidenceOf(needs, value)); }}>
          {sendLabel(needs)}
        </Button>
        <Button onClick={onClose}>{sending ? "Close" : "Cancel"}</Button>
      </span>
      <Saving sending={sending} />
    </div>
  );
}
