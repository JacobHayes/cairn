// B11, A19: a journey's status and what it accepts next: complete (suggested when nothing in
// scope is left or the final milestone is reached), reopen, archive, un-archive; and, once
// archived, hard deletion, which removes the journey and its history and is sent only with
// the journey's name typed back. Each is one patch on the revision shown.
import { useState } from "react";
import { useNavigate } from "react-router";

import type { Ready } from "../detail/model.ts";
import { Refused } from "../screens/Refused.tsx";
import { useScreenWrite } from "../screens/write.ts";
import { Button, Field, Panel } from "../ui/kit.tsx";
import { indexPath } from "./address.ts";
import { deleteConfirmed, statusActions } from "./lifecycle.ts";

function DeleteJourney({ ready }: { ready: Ready }) {
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
      </span>
      {write.rejected === undefined ? null : <Refused rejection={write.rejected} onDismiss={write.dismiss} />}
    </div>
  );
}

export function StatusPanel({ ready, suggested }: { ready: Ready; suggested: boolean }) {
  const { header, revision } = ready.journey;
  const write = useScreenWrite();
  const actions = statusActions(header.status);
  return (
    <Panel aria-label="Status" data-testid="status-panel">
      <span className="row">
        <strong>Status: {header.status}</strong>
        {header.status === "active" && suggested ? (
          <span className="badge badge-good" data-testid="completion-suggested">Everything in scope is finished: completion suggested</span>
        ) : null}
        {header.status === "archived" ? <span className="muted">Archived: it accepts only un-archiving or deletion.</span> : null}
      </span>
      <span className="row">
        {actions.map((action) => (
          <Button
            key={action.to}
            primary={action.to === "completed" && suggested}
            disabled={write.disabled}
            data-testid={`status-${action.to}`}
            onClick={() => void write.run({ target: { journey: header.id }, baseRevision: revision, mutations: [{ op: "set_journey_status", status: action.to }] })}
          >
            {action.label}
          </Button>
        ))}
      </span>
      {write.rejected === undefined ? null : <Refused rejection={write.rejected} onDismiss={write.dismiss} />}
      {header.status === "archived" ? <DeleteJourney ready={ready} /> : null}
    </Panel>
  );
}
