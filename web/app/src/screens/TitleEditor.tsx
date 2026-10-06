// One node's title edited through the shared write path: the draft (its text and the
// revision its author saw) is kept in session storage, so it survives a reload; saving sends
// one `set_node_field` patch against that revision (H5). A change elsewhere since is retried
// transparently; a change to the same node is shown, and the author saves again on the
// current revision or abandons the edit. Version skew disables saving and keeps the draft.
import { useState } from "react";

import { useDraft } from "../data/drafts.ts";
import { useSession, useSkew } from "../data/react.ts";
import type { Rejection, WriteResult } from "../data/writes.ts";
import { Button, Field } from "../ui/kit.tsx";
import { RejectionView } from "./RejectionView.tsx";

interface TitleDraft {
  text: string;
  /** The journey revision the author saw when they started editing. */
  base: number;
}

/**
 * The revision an author who keeps their edit after a conflict drafts it against: the
 * journey's revision the rejection reports (only what it shows was compared, H5), or the one
 * the view holds if it has moved further.
 */
export function rebasedOnto(rejection: Rejection, journey: string, revision: number): number {
  if (rejection.rejection !== "stale") {
    return revision;
  }
  const reported = rejection.conflicts.map((conflict) =>
    "domain" in conflict.of && typeof conflict.of.domain === "object" && "journey" in conflict.of.domain && conflict.of.domain.journey === journey
      ? conflict.current
      : 0,
  );
  return Math.max(revision, ...reported);
}

export interface TitleEditorProps {
  journey: string;
  node: string;
  title: string;
  /** The journey revision the view holds now. */
  revision: number;
}

export function TitleEditor({ journey, node, title, revision }: TitleEditorProps) {
  const session = useSession();
  const skew = useSkew();
  const [draft, setDraft] = useDraft<TitleDraft>(`title:${journey}:${node}`);
  const [rejection, setRejection] = useState<Rejection | undefined>();
  const [saving, setSaving] = useState(false);
  if (draft === undefined) {
    return (
      <span className="row">
        <span data-testid="title">{title}</span>
        <Button aria-label={`Rename ${title}`} onClick={() => { setDraft({ text: title, base: revision }); }}>
          Rename
        </Button>
      </span>
    );
  }
  const save = async () => {
    setSaving(true);
    const result: WriteResult = await session.write({
      target: { journey },
      baseRevision: draft.base,
      mutations: [{ op: "set_node_field", node, value: { title: draft.text } }],
    });
    setSaving(false);
    setRejection(result.outcome === "rejected" ? result.rejection : undefined);
    if (result.outcome === "landed") {
      setDraft(undefined);
    }
  };
  return (
    <span className="stack">
      <span className="row">
        <Field
          aria-label={`New title for ${title}`}
          value={draft.text}
          onChange={(event) => { setDraft({ ...draft, text: event.target.value }); }}
        />
        <Button primary disabled={saving || skew !== undefined} onClick={() => void save()}>
          Save
        </Button>
        <Button onClick={() => { setDraft(undefined); setRejection(undefined); }}>Cancel</Button>
      </span>
      {draft.base < revision ? <span className="muted">Editing as of revision {draft.base}; now {revision}.</span> : null}
      {rejection === undefined ? null : (
        <RejectionView
          rejection={rejection}
          onRebase={() => {
            setDraft({ ...draft, base: rebasedOnto(rejection, journey, revision) });
            setRejection(undefined);
          }}
        />
      )}
    </span>
  );
}
