// E6: merging one identity into another, one deployment patch. Every journey referring to
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
import { RejectionView } from "../screens/RejectionView.tsx";
import { WriteStatus } from "../screens/WriteStatus.tsx";
import { Failed } from "../ui/Failed.tsx";
import { Button } from "../ui/kit.tsx";
import { Receipt, useReceipt } from "../ui/Receipt.tsx";
import { identityLabel } from "./label.ts";
import { mergeFlow } from "./merge.ts";
import type { Identity } from "./model.ts";

/** An identity's label by key (its name, else its first email). */
function nameOf(identities: readonly Identity[], key: string): string {
  const found = identities.find((identity) => identity.key === key);
  return found === undefined ? "" : identityLabel(found);
}

function IdentitySelect({ label, value, identities, onChange }: { label: string; value: string; identities: readonly Identity[]; onChange: (key: string) => void }) {
  return (
    <label className="field">
      <span className="label">{label}</span>
      <select aria-label={label} value={value} onChange={(event) => { onChange(event.target.value); }}>
        <option value="">Choose an identity</option>
        {identities.map((identity) => (
          <option key={identity.key} value={identity.key}>
            {identity.name === undefined || identity.name === null ? identityLabel(identity) : `${identity.name}${(identity.emails ?? []).length === 0 ? "" : ` (${(identity.emails ?? []).join(", ")})`}`}
          </option>
        ))}
      </select>
    </label>
  );
}

export function MergeIdentities({ deployment, identities }: { deployment: Deployment; identities: readonly Identity[] }) {
  const { host, edits, titleOf } = useSession();
  const skew = useSkew();
  const flow = useActorRef(mergeFlow, {
    input: {
      edits,
      key: "merge-identities",
      referencing: (keys) => allPages<JourneySummary, string>((after) => host.journeys({ referencing: [...keys], ...(after === undefined ? {} : { after }) })),
    },
  });
  const working = useSelector(flow, (snapshot) => snapshot.hasTag("busy"));
  const failure = useSelector(flow, (snapshot) => snapshot.context.failure);
  const write = useEdit("merge-identities");
  const receipt = useReceipt();
  const [survivor, setSurvivor] = useState("");
  const [merged, setMerged] = useState("");
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
    <section id="merge-identities" className="node-panel" aria-label="Merge two identities" data-testid="merge-identities">
      <header className="node-panel-head">
        <h2 className="node-panel-title">Merge two identities</h2>
      </header>
      <div className="stack merge-body">
      <span className="muted small">Journeys and their history will show the identity you keep.</span>
      {ready ? <span data-testid="merge-sentence">{nameOf(identities, merged)} will be merged into {nameOf(identities, survivor)}. {nameOf(identities, survivor)} will remain.</span> : null}
      <span className="row merge-fields">
        <IdentitySelect label="Keep this identity" value={survivor} identities={identities} onChange={setSurvivor} />
        <IdentitySelect label="Merge this identity into it" value={merged} identities={identities} onChange={setMerged} />
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
        <RejectionView rejection={write.rejection} onDismiss={write.dismiss} />
      )}
      </div>
    </section>
  );
}
