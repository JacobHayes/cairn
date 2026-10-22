// A19: hard deletion of archived journeys, one or several: one patch to the deployment naming
// each journey at the revision shown, so a journey that moved since makes the whole patch stale
// and nothing is deleted. The journey header's `⋯` and the journey index's selection both send
// it through here, asked once with the count. The status changes themselves (complete, reopen,
// archive, un-archive: B11) are under the journey header's `⋯`, Status (screens/JourneyHeader.tsx).
import { useState } from "react";
import { useNavigate } from "react-router";

import { useDeployment } from "../data/react.ts";
import type { Ready } from "../detail/model.ts";
import { RejectionView } from "../screens/RejectionView.tsx";
import { WriteStatus } from "../screens/WriteStatus.tsx";
import { useScreenWrite } from "../screens/write.ts";
import { Button } from "../ui/kit.tsx";
import { indexPath } from "./address.ts";
import { deleteBlocked, deleteLabel, deleteMutations, deleteQuestion, type Doomed } from "./lifecycle.ts";

interface DeleteProps {
  journeys: readonly Doomed[];
  /** After the deletion landed. */
  onDeleted: () => void;
  /** Ask at once, with this to go back (the header's panel, opened from its `⋯`); without it a Delete button asks first. */
  onCancel?: () => void;
}

/** Delete, why it is off, or the one question before it goes. */
export function DeleteJourneys({ journeys, onDeleted, onCancel }: DeleteProps) {
  const deployment = useDeployment();
  const [only] = journeys;
  const write = useScreenWrite("delete-journeys", journeys.length === 1 && only !== undefined ? only.name : `${String(journeys.length)} journeys`);
  const [asking, setAsking] = useState(false);
  const blocked = deleteBlocked(journeys);
  const asked = onCancel !== undefined || asking;
  const remove = async () => {
    if (deployment === undefined) {
      return;
    }
    if (await write.run({ target: "deployment", baseRevision: deployment.revision, mutations: deleteMutations(journeys) })) {
      setAsking(false);
      onDeleted();
    }
  };
  const status = write.rejection === undefined ? <WriteStatus view={write.status} /> : <RejectionView rejection={write.rejection} onDismiss={write.dismiss} />;
  if (blocked !== undefined || !asked) {
    return (
      <div className="stack delete-group" data-testid="delete-journeys" data-blocked={blocked !== undefined}>
        <p className="muted" data-testid="delete-why">{blocked ?? "Deleting removes a journey and all of its history for good."}</p>
        <span className="row">
          <Button disabled={blocked !== undefined || write.disabled} data-testid="delete-ask" onClick={() => { setAsking(true); }}>
            {deleteLabel(journeys.length)}
          </Button>
        </span>
        {status}
      </div>
    );
  }
  return (
    <div className="stack callout" role="alertdialog" aria-label={deleteQuestion(journeys)} data-testid="delete-confirm" data-count={journeys.length}>
      <strong>{deleteQuestion(journeys)}</strong>
      <span>{journeys.length === 1 ? "It is removed with all of its history" : "Each is removed with all of its history"}; only a record of who deleted {journeys.length === 1 ? "it" : "each"} and when is kept. This cannot be undone.</span>
      <span className="row">
        <Button primary disabled={write.disabled || deployment === undefined} data-testid="delete-confirmed" onClick={() => void remove()}>
          {deleteLabel(journeys.length)}
        </Button>
        <Button onClick={() => { setAsking(false); onCancel?.(); }}>Cancel</Button>
      </span>
      {status}
    </div>
  );
}

/** The journey header's deletion of its own journey, asked at once; once it is gone the index (every status) is shown. */
export function DeleteJourney({ ready, onCancel }: { ready: Ready; onCancel: () => void }) {
  const { header, revision } = ready.journey;
  const navigate = useNavigate();
  const journeys = [{ id: header.id, name: header.name, status: header.status, revision }];
  return <DeleteJourneys journeys={journeys} onCancel={onCancel} onDeleted={() => void navigate(indexPath({ status: "any", route: undefined, version: undefined, mine: false, upgrade: false }))} />;
}
