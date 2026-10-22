// C11, G2, G4, G5: finishing work that requires a note, a link or its checklist ticked and lacks
// it. Mark done... opens what it needs; sending adds it and completes in one patch, so the guards
// see it. The inspector's Actions open it for every surface (a row opens the inspector's), so a
// Complete that the engine would reject is never the primary action.
import { useEffect, useRef } from "react";

import { newAttachmentKey } from "./Attachments.tsx";
import { evidenceComplete, type Evidence, type EvidenceDraft, type Needs } from "../acting/acts.ts";
import { Button, Field } from "../ui/kit.tsx";
import { Saving, type Sending } from "../ui/Receipt.tsx";
import { plural } from "../ui/words.ts";

/** The words on the button that adds what done needs and completes: one need names it, several confirm the whole form. */
function sendLabel({ artifact, note, unticked }: Needs): string {
  if (Number(artifact) + Number(note) + Number(unticked.length > 0) !== 1) {
    return "Mark done";
  }
  if (unticked.length > 0) {
    return `Tick ${plural(unticked.length, "item")} and mark done`;
  }
  return artifact ? "Add link and mark done" : "Add note and mark done";
}

/** What sending `draft` adds before completing: each field the node needs, trimmed, with its new key, and every item shown as unticked. */
export function evidenceOf(needs: Needs, draft: EvidenceDraft): Evidence {
  return {
    ...(needs.artifact ? { artifact: { key: newAttachmentKey(), url: draft.artifact.trim() } } : {}),
    ...(needs.note ? { note: { key: newAttachmentKey(), text: draft.note.trim() } } : {}),
    ...(needs.unticked.length > 0 ? { ticks: needs.unticked } : {}),
  };
}

/** G5: the checklist items sending ticks, every one by title: the button certifies all of them. */
function UntickedList({ titles }: { titles: string[] }) {
  return (
    <>
      <p data-testid="done-ticks">
        {plural(titles.length, "checklist item")} {titles.length === 1 ? "is" : "are"} not ticked:
      </p>
      <ul className="done-ticks-list">
        {titles.map((title, at) => (
          <li key={at}>{title}</li>
        ))}
      </ul>
    </>
  );
}

/**
 * The fields done needs and the button that sends them. The form holds no state and sends
 * nothing itself: its owner keeps the draft, submits the write, and says when one is out
 * (`sending`: Cancel reads Close, because closing leaves the save going). `unticked` are the
 * titles of the items sending ticks, all of them, because the button certifies each; `onWithout`
 * switches to finishing without them, with a reason.
 */
export function EvidenceForm({
  needs,
  unticked,
  value,
  sending,
  disabled,
  onChange,
  onSend,
  onWithout,
  onClose,
}: {
  needs: Needs;
  unticked: string[];
  value: EvidenceDraft;
  sending: Sending;
  disabled: boolean;
  onChange: (next: EvidenceDraft) => void;
  onSend: (evidence: Evidence) => void;
  onWithout: (() => void) | undefined;
  onClose: () => void;
}) {
  const { artifact, note } = value;
  const ticks = needs.unticked.length > 0;
  // A form opened low in a short sheet or a folded card is brought into view; with nothing to type the button takes the focus.
  const shown = useRef<HTMLDivElement>(null);
  useEffect(() => {
    shown.current?.scrollIntoView({ block: "nearest" });
  }, []);
  return (
    <div className="stack row-more" data-testid="done-evidence" ref={shown}>
      {ticks ? <UntickedList titles={unticked} /> : null}
      {needs.artifact ? (
        <label className="stack">
          <span className="label">Artifact link</span>
          <Field autoFocus value={artifact} onChange={(event) => { onChange({ artifact: event.target.value, note }); }} />
        </label>
      ) : null}
      {needs.note ? (
        <label className="stack">
          <span className="label">Note</span>
          <textarea autoFocus={!needs.artifact} value={note} onChange={(event) => { onChange({ artifact, note: event.target.value }); }} />
        </label>
      ) : null}
      <span className="row">
        <Button primary autoFocus={ticks && !needs.artifact && !needs.note} disabled={disabled || !evidenceComplete(needs, { artifact, note })} onClick={() => { onSend(evidenceOf(needs, value)); }}>
          {sendLabel(needs)}
        </Button>
        <Button onClick={onClose}>{sending ? "Close" : "Cancel"}</Button>
      </span>
      {ticks && onWithout !== undefined ? (
        <span>
          <Button data-testid="done-without" disabled={disabled} onClick={onWithout}>
            Mark done without them…
          </Button>
        </span>
      ) : null}
      <Saving sending={sending} />
    </div>
  );
}
