// E6: merging one entity into another, one deployment patch. Every journey referring to
// either (any status, archived among them) is read first and named at the revision seen, so
// the host checks each as it would derive after the merge and rechecks the set when it
// commits; a merge that would break a journey is refused naming it and its violations.
import { useState } from "react";

import type { Deployment } from "../data/host.ts";
import { allPages } from "../data/live.ts";
import type { JourneySummary } from "../data/reads.ts";
import { useSession } from "../data/react.ts";
import { Refused } from "../screens/Refused.tsx";
import { useScreenWrite } from "../screens/write.ts";
import { Button } from "../ui/kit.tsx";
import { mergeMutation, type Entity } from "./model.ts";

function EntitySelect({ label, value, entities, onChange }: { label: string; value: string; entities: readonly Entity[]; onChange: (key: string) => void }) {
  return (
    <label className="row">
      <span className="muted">{label}</span>
      <select className="select" aria-label={label} value={value} onChange={(event) => { onChange(event.target.value); }}>
        <option value="">Choose an entity</option>
        {entities.map((entity) => (
          <option key={entity.key} value={entity.key}>
            {entity.name} ({entity.key})
          </option>
        ))}
      </select>
    </label>
  );
}

export function MergeEntities({ deployment, entities, asked }: { deployment: Deployment; entities: readonly Entity[]; asked: string[] }) {
  const { host, notices } = useSession();
  const write = useScreenWrite();
  const [survivor, setSurvivor] = useState(asked[0] ?? "");
  const [merged, setMerged] = useState(asked[1] ?? "");
  const [reading, setReading] = useState(false);
  const merge = async () => {
    setReading(true);
    let referencing: JourneySummary[];
    try {
      referencing = await allPages<JourneySummary, string>((after) => host.journeys({ referencing: [survivor, merged], ...(after === undefined ? {} : { after }) }));
    } catch (thrown) {
      notices.add({ tone: "problem", title: `Not merged: ${thrown instanceof Error ? thrown.message : String(thrown)}`, lines: [] });
      return;
    } finally {
      setReading(false);
    }
    if (await write.run({ target: "deployment", baseRevision: deployment.revision, mutations: [mergeMutation(survivor, merged, referencing)] })) {
      setMerged("");
    }
  };
  const ready = survivor !== "" && merged !== "" && survivor !== merged;
  return (
    <section className="stack panel" aria-label="Merge two entities" data-testid="merge-entities">
      <strong>Merge two entities</strong>
      <span className="muted">The second becomes an alias of the first: journeys and their history keep referring to it and now read the first.</span>
      <span className="row">
        <EntitySelect label="Keep" value={survivor} entities={entities} onChange={setSurvivor} />
        <EntitySelect label="Merge into it" value={merged} entities={entities} onChange={setMerged} />
        <Button primary disabled={!ready || reading || write.disabled} onClick={() => void merge()}>Merge</Button>
      </span>
      {write.rejected === undefined ? null : <Refused rejection={write.rejected} onDismiss={write.dismiss} />}
    </section>
  );
}
