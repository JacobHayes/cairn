// C16: the owners field of the journey forms: the identities chosen as chips (the viewer's own
// reads "You"), each removable, and a select that adds one more from the deployment's identities.
// Owners are existing identities only; the field stops at the shared limit of identities a list holds.
import { IDENTITY_COUNT_PER_FILL_MAX } from "@cairn/client";

import { useCaller, useDeployment } from "../data/react.ts";
import { resolveAlias } from "../detail/sections.tsx";
import { identityLabel } from "../people/label.ts";
import { Chip, Cross } from "../ui/Chip.tsx";

/** The keys of `owners` once each, through any merge. */
export function distinctOwners(owners: readonly string[], resolve: (key: string) => string): string[] {
  return [...new Set(owners.map(resolve))];
}

/** `label` is the label's class: the editor's mono label above the box, or none beside a form grid's plain labels. */
export function OwnersField({ owners, onChange, label }: { owners: readonly string[]; onChange: (owners: string[]) => void; label?: string }) {
  const deployment = useDeployment();
  const { caller } = useCaller();
  if (deployment === undefined) {
    return null;
  }
  const identities = deployment.identities ?? [];
  // The viewer's own identity reads "You"; any other identity of theirs is chosen by its name.
  const own = (caller?.identities ?? [])[0] === undefined ? undefined : resolveAlias(deployment, (caller?.identities ?? [])[0] ?? "");
  const nameOf = (key: string) => {
    const identity = identities.find((each) => each.key === key);
    return key === own ? "You" : identity === undefined ? key : identityLabel(identity);
  };
  const chosen = distinctOwners(owners, (key) => resolveAlias(deployment, key));
  const offered = identities
    .filter((identity) => !chosen.includes(identity.key))
    .map((identity) => ({ key: identity.key, label: nameOf(identity.key) }))
    .sort((left, right) => left.label.localeCompare(right.label));
  const full = chosen.length >= IDENTITY_COUNT_PER_FILL_MAX;
  return (
    <>
      <span className={label} id="owners-label">Owners</span>
      <div className="chips-input" role="group" aria-labelledby="owners-label" data-testid="owners-field">
        {chosen.map((key) => {
          const name = nameOf(key);
          return (
            <Chip
              key={key}
              remove={
                <button type="button" aria-label={`Remove ${name}`} onClick={() => { onChange(chosen.filter((each) => each !== key)); }}>
                  <Cross />
                </button>
              }
            >
              {name}
            </Chip>
          );
        })}
        <select
         
          aria-label="Add an owner"
          value=""
          disabled={full || offered.length === 0}
          onChange={(event) => { if (event.target.value !== "") { onChange([...chosen, event.target.value]); } }}
        >
          <option value="">{full ? `${String(IDENTITY_COUNT_PER_FILL_MAX)} owners at most` : "Add an owner"}</option>
          {offered.map((each) => (
            <option key={each.key} value={each.key}>{each.label}</option>
          ))}
        </select>
      </div>
    </>
  );
}
