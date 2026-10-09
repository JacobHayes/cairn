// A19: hard deletion of an archived journey, which removes the journey and its history and is
// sent only with the journey's name typed back. One patch on the revision shown. The status
// changes themselves (complete, reopen, archive, un-archive: B11) are the lifecycle chip's
// menu in the journey header (screens/JourneyHeader.tsx).
import { useState } from "react";
import { useNavigate } from "react-router";

import type { Ready } from "../detail/model.ts";
import { Refused } from "../screens/Refused.tsx";
import { useScreenWrite } from "../screens/write.ts";
import { Button, Field } from "../ui/kit.tsx";
import { indexPath } from "./address.ts";
import { deleteConfirmed } from "./lifecycle.ts";

export function DeleteJourney({ ready, onCancel }: { ready: Ready; onCancel: () => void }) {
  const { header, revision } = ready.journey;
  const navigate = useNavigate();
  const write = useScreenWrite();
  const [typed, setTyped] = useState("");
  const remove = async () => {
    if (await write.run({ target: { journey: header.id }, baseRevision: revision, mutations: [{ op: "delete_journey" }] })) {
      void navigate(indexPath({ status: "any", route: undefined, version: undefined, mine: false, upgrade: false }));
    }
  };
  return (
    <div className="stack callout" data-testid="delete-journey">
      <strong>Delete this journey for good</strong>
      <span>Deleting removes the journey and all of its history; only a record of who deleted it and when is kept. Type its name to confirm.</span>
      <Field aria-label="Type the journey's name to delete it" value={typed} onChange={(event) => { setTyped(event.target.value); }} />
      <span className="row">
        <Button disabled={write.disabled || !deleteConfirmed(header.status, header.name, typed)} onClick={() => void remove()}>
          Delete the journey
        </Button>
        <Button onClick={onCancel}>Cancel</Button>
      </span>
      {write.rejected === undefined ? null : <Refused rejection={write.rejected} onDismiss={write.dismiss} />}
    </div>
  );
}
