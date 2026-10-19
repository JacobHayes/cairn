// E6: merging one entity into another, one deployment patch. Every journey referring to
// either (any status, archived among them) is read first and named at the revision seen, so
// the host checks each as it would derive after the merge and rechecks the set when it
// commits; a merge that would break a journey is refused naming it and its violations.
import { useActorRef, useSelector } from "@xstate/react";
import { useEffect, useState } from "react";

import type { Deployment } from "../data/host.ts";
import { allPages } from "../data/live.ts";
import type { JourneySummary } from "../data/reads.ts";
import { useEdit, useSession, useSkew } from "../data/react.ts";
import { landedWarning } from "../data/writes.ts";
import { Refused } from "../screens/Refused.tsx";
import { WriteStatus } from "../screens/WriteStatus.tsx";
import { Failed } from "../ui/Failed.tsx";
import { Button } from "../ui/kit.tsx";
import { Receipt, useReceipt } from "../ui/Receipt.tsx";
import { mergeFlow } from "./merge.ts";
import type { Entity } from "./model.ts";

/** An entity's name by key. */
function nameOf(entities: readonly Entity[], key: string): string {
  return entities.find((entity) => entity.key === key)?.name ?? "";
}

function EntitySelect({ label, value, entities, onChange }: { label: string; value: string; entities: readonly Entity[]; onChange: (key: string) => void }) {
  return (
    <label className="field">
      <span className="label">{label}</span>
      <select aria-label={label} value={value} onChange={(event) => { onChange(event.target.value); }}>
        <option value="">Choose an entity</option>
        {entities.map((entity) => (
          <option key={entity.key} value={entity.key}>
            {entity.name}
            {(entity.emails ?? []).length === 0 ? "" : ` (${(entity.emails ?? []).join(", ")})`}
          </option>
        ))}
      </select>
    </label>
  );
}

export function MergeEntities({ deployment, entities, asked }: { deployment: Deployment; entities: readonly Entity[]; asked: string[] }) {
  const { host, edits, titleOf } = useSession();
  const skew = useSkew();
  const flow = useActorRef(mergeFlow, {
    input: {
      edits,
      key: "merge-entities",
      referencing: (keys) => allPages<JourneySummary, string>((after) => host.journeys({ referencing: [...keys], ...(after === undefined ? {} : { after }) })),
    },
  });
  const working = useSelector(flow, (snapshot) => snapshot.hasTag("busy"));
  const failure = useSelector(flow, (snapshot) => snapshot.context.failure);
  const write = useEdit("merge-entities");
  const receipt = useReceipt();
  const [survivor, setSurvivor] = useState(asked[0] ?? "");
  const [merged, setMerged] = useState(asked[1] ?? "");
  // Told of a landing only while this control is on screen.
  useEffect(() => {
    const subscription = flow.on("merged", ({ answer }) => {
      setMerged("");
      receipt.show(landedWarning(answer, titleOf));
    });
    return () => {
      subscription.unsubscribe();
    };
  });
  const ready = survivor !== "" && merged !== "" && survivor !== merged;
  return (
    <section id="merge-entities" className="node-panel" aria-label="Merge two entities" data-testid="merge-entities">
      <header className="node-panel-head">
        <h2 className="node-panel-title">Merge two entities</h2>
      </header>
      <div className="stack merge-body">
      <span className="muted small">Journeys and their history will show the entity you keep.</span>
      {ready ? <span data-testid="merge-sentence">{nameOf(entities, merged)} will be merged into {nameOf(entities, survivor)}. {nameOf(entities, survivor)} will remain.</span> : null}
      <span className="row merge-fields">
        <EntitySelect label="Keep this entity" value={survivor} entities={entities} onChange={setSurvivor} />
        <EntitySelect label="Merge this entity into it" value={merged} entities={entities} onChange={setMerged} />
        <Button
          primary
          disabled={!ready || working || write.busy || skew}
          onClick={() => {
            receipt.clear();
            flow.send({ type: "merge", survivor, merged, deployment: deployment.revision });
          }}
        >
          Merge
        </Button>
      </span>
      {failure === undefined ? null : <p className="callout callout-bad" role="alert" data-testid="merge-failed"><Failed what="The merge" verb="made" error={failure} /></p>}
      {write.rejection === undefined ? (
        <>
          <WriteStatus view={write.status} />
          <Receipt receipt={receipt.receipt} />
        </>
      ) : (
        <Refused rejection={write.rejection} onDismiss={write.dismiss} />
      )}
      </div>
    </section>
  );
}
