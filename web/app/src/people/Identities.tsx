// E6, B3, H3: the deployment's identities, as a table in the index shape (the title with Add at
// its right end, one toolbar row under it). A row opens that identity alone in the inspector
// (`/identities/<key>`); the row checkboxes, and Shift- or Cmd-click, add and remove from one set
// (canvas/selection.ts, the same set a journey's List keeps) and several are merged or deleted
// from the inspector (IdentityPanels.tsx). Add opens its form in the inspector (`/identities/new`).
// Under a proposal's layer the table draws the identities after it, each change marked, and
// nothing can be selected or written. The deployment's open proposals are the inspector's
// Proposals tab. Kept current (H6).
import { useState, type MouseEvent, type ReactNode } from "react";
import { Link, useNavigate, useParams } from "react-router";

import { pathTo, usePage } from "../address.ts";
import { useSelection, type Selection } from "../canvas/selection.ts";
import type { Deployment } from "../data/host.ts";
import { useCaller, useDeployment } from "../data/react.ts";
import { Band } from "../proposals/Band.tsx";
import { DestinationProposals } from "../proposals/DestinationProposals.tsx";
import { DeploymentLayerHost } from "../proposals/Hosts.tsx";
import { identityDiff, useLayer, useReviewing, type IdentityRow } from "../proposals/layer.ts";
import { DIFF_LABELS } from "../proposals/model.ts";
import { Rail } from "../proposals/Rail.tsx";
import { useWritable } from "../screens/writable.ts";
import { Inspector } from "../shell/frame.tsx";
import { useEscapeTo } from "../shell/useEscapeTo.ts";
import { FilterSearch, IndexHead } from "../ui/IndexHead.tsx";
import { Badge, IndexState } from "../ui/kit.tsx";
import { Receipt, useReceipt } from "../ui/Receipt.tsx";
import { plural } from "../ui/words.ts";
import { NewIdentity } from "./IdentityForm.tsx";
import { Emails, NoSuchIdentity, OneIdentity, SeveralIdentities, useMerge } from "./IdentityPanels.tsx";
import { identityLabel } from "./label.ts";
import { byName, type Identity } from "./model.ts";
import { useReferences } from "./references.ts";

/** Whether `identity` matches the search: its name or any of its emails contains the text, without regard to case. */
export function matches(identity: Identity, search: string): boolean {
  const wanted = search.trim().toLowerCase();
  return wanted === "" || [identity.name ?? "", ...(identity.emails ?? [])].some((text) => text.toLowerCase().includes(wanted));
}

/** The count line: the identities now, and under a proposal also how many it leaves; a search says how many of them it keeps. */
export function countWords(args: { total: number; shown: number; after: number | undefined }): string {
  const { total, shown, after } = args;
  const now = plural(total, "identity", "identities");
  if (after !== undefined) {
    return `${now} now, ${String(after)} after`;
  }
  return shown === total ? now : `${String(shown)} of ${now}`;
}

/** A click on the row itself, not on a control or link inside it. */
function onRow(event: MouseEvent): boolean {
  return !(event.target instanceof Element && event.target.closest("a, button, select, input, textarea, label"));
}

function Row({ row, yours, chosen, intoName, selection, selectable }: { row: IdentityRow; yours: boolean; chosen: boolean; intoName: string | undefined; selection: Selection; selectable: boolean }) {
  const { identity, mark } = row;
  const label = identityLabel(identity);
  const named = identity.name !== undefined && identity.name !== null;
  const modified = (event: MouseEvent) => event.shiftKey || event.metaKey || event.ctrlKey;
  return (
    <tr
      data-testid="identity"
      data-identity={identity.key}
      data-selected={chosen}
      data-diff={mark?.kind}
      onClick={(event) => {
        if (!selectable || !onRow(event)) {
          return;
        }
        if (modified(event)) {
          selection.toggle(identity.key);
        } else {
          selection.pick(identity.key);
        }
      }}
    >
      <td className="pick">
        {selectable ? <input type="checkbox" className="pick-box" aria-label={`Select ${label}`} checked={chosen} onChange={() => { selection.toggle(identity.key); }} /> : null}
      </td>
      <td data-column="name">
        <span className="cell-first">
          {mark === undefined ? null : (
            <span className="layer-word" data-testid="row-diff" data-diff={mark.kind}>
              {mark.into === undefined ? DIFF_LABELS[mark.kind] : "Merged"}
            </span>
          )}
          {selectable ? (
            <Link
              to={pathTo("identity", { key: identity.key })}
              data-testid="identity-name"
              className={named ? undefined : "mono"}
              onClick={(event) => {
                // A modified click adds to the set, a plain one opens this identity alone; neither is left to the address, which may be this one already.
                event.preventDefault();
                event.stopPropagation();
                if (modified(event)) {
                  selection.toggle(identity.key);
                } else {
                  selection.pick(identity.key);
                }
              }}
            >
              {label}
            </Link>
          ) : (
            <strong data-testid="identity-name" className={named ? undefined : "mono"}>{label}</strong>
          )}
          {yours ? <Badge tone="good" data-testid="yours">You</Badge> : null}
        </span>
        {mark?.into === undefined ? null : <span className="cell-second" data-testid="merged-into">Into {intoName ?? "another identity"}</span>}
      </td>
      <td data-column="emails">
        <Emails identity={identity} />
      </td>
    </tr>
  );
}

/** What the inspector holds: Add's form, the one identity open, the several selected, or that the address names no identity. Nothing under a proposal's layer, whose rail is the inspector's. */
function InspectorOf({ deployment, selection, reviewing, adding, address }: { deployment: Deployment; selection: Selection; reviewing: boolean; adding: boolean; address: string | undefined }) {
  const { caller } = useCaller();
  const receipt = useReceipt();
  const merging = useMerge(receipt, selection);
  const yours = new Set(caller?.identities ?? []);
  const held = new Map((deployment.identities ?? []).map((identity) => [identity.key, identity]));
  const members = selection.members.map((key) => held.get(key)).filter((identity): identity is Identity => identity !== undefined);
  const references = useReferences(reviewing || adding ? [] : members.map((member) => member.key));
  const one = members.length === 1 ? members[0] : undefined;
  if (reviewing) {
    return null;
  }
  let focus = address;
  let panel: ReactNode = <NoSuchIdentity />;
  if (adding) {
    focus = "new-identity";
    panel = <NewIdentity deployment={deployment} />;
  } else if (one !== undefined) {
    focus = one.key;
    panel = <OneIdentity identity={one} deployment={deployment} yours={yours.has(one.key)} reading={references.reading} held={references.held?.[one.key]} refresh={references.refresh} selection={selection} />;
  } else if (members.length > 1) {
    focus = members.map((member) => member.key).join(" ");
    panel = <SeveralIdentities members={members} deployment={deployment} yours={yours} reading={references.reading} refresh={references.refresh} selection={selection} receipt={receipt} merging={merging} />;
  }
  // The receipt is held here, not in the panel, so a merge's warnings outlive the panel it replaces.
  return focus === undefined ? null : (
    <Inspector focus={focus}>
      {panel}
      <Receipt receipt={receipt.receipt} />
    </Inspector>
  );
}

/** The table of `shown`, the identities the search keeps; under a layer its rows are marked and none can be selected. */
function IdentityTable({ shown, names, selection, reviewing }: { shown: IdentityRow[]; names: ReadonlyMap<string, string>; selection: Selection; reviewing: boolean }) {
  const { caller } = useCaller();
  const yours = new Set(caller?.identities ?? []);
  return (
    <div className="index-table identities-table" data-testid="identity-list" data-selecting={selection.members.length > 0}>
      <table className="data" aria-label="Identities">
        <thead>
          <tr>
            <th scope="col" className="pick">
              <span className="visually-hidden">Select</span>
            </th>
            <th scope="col">Name</th>
            <th scope="col">Emails</th>
          </tr>
        </thead>
        <tbody>
          {shown.map((row) => (
            <Row
              key={row.identity.key}
              row={row}
              yours={yours.has(row.identity.key)}
              chosen={selection.members.includes(row.identity.key)}
              intoName={row.mark?.into === undefined ? undefined : names.get(row.mark.into)}
              selection={selection}
              selectable={!reviewing}
            />
          ))}
        </tbody>
      </table>
    </div>
  );
}

/** The identities page: the identities as they are, or, with a proposal's layer on, as it would leave them with every change marked. */
function IdentitiesPage({ deployment, rows }: { deployment: Deployment; rows: IdentityRow[] | undefined }) {
  const layer = useLayer();
  const reviewing = useReviewing() !== undefined;
  // Any proposal's layer leaves the page without a live write: the rail, not the inspector, speaks.
  const live = useWritable();
  const navigate = useNavigate();
  const adding = usePage()?.name === "newIdentity";
  const address = useParams()["key"];
  const selection = useSelection(live ? address : undefined, (key) => void navigate(key === undefined ? pathTo("identities") : pathTo("identity", { key })));
  const [search, setSearch] = useState("");
  useEscapeTo(!live || (address === undefined && !adding) ? undefined : pathTo("identities"));
  const listed: IdentityRow[] = rows ?? byName(deployment.identities ?? []).map((identity) => ({ identity, mark: undefined }));
  const shown = listed.filter((row) => matches(row.identity, search));
  const after = reviewing ? listed.filter((row) => row.mark?.kind !== "removed").length : undefined;
  const add = (
    <Link className="button" to={pathTo("newIdentity")} data-testid="new-identity">
      Add
    </Link>
  );
  return (
    <section className="stack index-page" aria-label="Identities" data-testid="identities" data-revision={deployment.revision} data-layer={layer?.mode} data-rows-page tabIndex={-1}>
      <IndexHead title="Identities" action={live ? add : undefined}>
        <FilterSearch label="Search" placeholder="Name or email" value={search} onChange={setSearch} />
        <span className="index-count" data-testid="identities-count">{countWords({ total: (deployment.identities ?? []).length, shown: shown.length, after })}</span>
      </IndexHead>
      <Band />
      {reviewing ? null : <p className="muted index-intro">People and teams. What each does in a journey is the role it fills there.</p>}
      {listed.length === 0 ? (
        <IndexState kind="empty" action={!live ? undefined : <Link to={pathTo("newIdentity")}>Add an identity &rsaquo;</Link>} data-testid="identities-empty">
          No identities yet.
        </IndexState>
      ) : shown.length === 0 ? (
        <IndexState kind="empty" data-testid="identities-none-match">
          No identity matches that search.
        </IndexState>
      ) : (
        <IdentityTable shown={shown} names={new Map(listed.map((row) => [row.identity.key, identityLabel(row.identity)]))} selection={selection} reviewing={!live} />
      )}
      <InspectorOf deployment={deployment} selection={selection} reviewing={!live} adding={adding} address={address} />
    </section>
  );
}

/** Under the layer, the rows diffed (in the order the page lists them) and the rail beside them. */
function LayeredIdentities() {
  const layer = useLayer();
  const deployment = layer?.deployment;
  if (layer === undefined || deployment === undefined) {
    return null;
  }
  const rows = layer.mode === "reviewing" ? identityDiff(deployment.before, deployment.after) : undefined;
  return (
    <>
      <IdentitiesPage deployment={deployment.before} rows={rows} />
      {layer.review === undefined ? null : <Rail selected={undefined} />}
    </>
  );
}

export function Identities() {
  const deployment = useDeployment();
  const proposal = useParams()["proposal"];
  if (deployment === undefined) {
    return <p className="muted">Reading the identities...</p>;
  }
  return (
    <>
      {proposal === undefined ? <IdentitiesPage deployment={deployment} rows={undefined} /> : <DeploymentLayerHost proposal={proposal}>{() => <LayeredIdentities />}</DeploymentLayerHost>}
      <DestinationProposals destination="deployment" />
    </>
  );
}
