// C8, D8: the inspector's header: the kind and where the node sits (each crumb a
// link) with its display state at the top right, its title (renamed in place from the menu),
// one meta line (the owner; the date and rank are the Dates section's and the sentence's), the flags that are set, and, for an orphaned node, the choice it needs. Where the
// node came from is not here: it is the Origin section.
import { useId } from "react";
import { Link } from "react-router";

import { CheckFailed, invalidAttrs, Problem } from "../authoring/parts.tsx";

import { RejectionView } from "../screens/RejectionView.tsx";
import { WriteStatus } from "../screens/WriteStatus.tsx";
import { useCaller } from "../data/react.ts";
import { statusTone, statusWord } from "../status/words.ts";
import { Badge, Button, Input } from "../ui/kit.tsx";
import { Receipt } from "../ui/Receipt.tsx";
import { flagsOf, isStage, kindWord, type NodeDetail, type Ready } from "./model.ts";
import { NodeLink, OwnerSelect } from "./parts.tsx";
import { ownersOf, ownerWords } from "./sections.tsx";
import type { Rename } from "./rename.ts";
import { rebasedOnto, useNodeWrite } from "./write.ts";

/** The title as an input: saving sends one `set_node_field` against the revision it was drafted on (H5), and from then on the edit manager owns the write. */
function RenameForm({ view, title, rename }: { view: Ready; title: string; rename: Rename }) {
  const { write, text, base, busy } = rename;
  const message = useId();
  return (
    <div className="stack" data-testid="rename">
      <span className="row">
        <Input
          aria-label={`New title for ${title}`}
          {...invalidAttrs(message, rename.problem)}
          value={text}
          disabled={busy || write?.state === "gone"}
          onChange={(event) => { rename.change(event.target.value); }}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              rename.save();
            }
          }}
        />
        <Button primary disabled={!rename.canSave} onClick={rename.save}>
          Save
        </Button>
        {busy ? null : <Button onClick={rename.cancel}>Cancel</Button>}
      </span>
      {rename.problem === undefined ? null : <Problem id={message}>{rename.problem}</Problem>}
      <CheckFailed failure={rename.failure} retry={rename.retry} />
      {base !== undefined && base < view.journey.revision ? <span className="muted small">The journey has changed since you started this edit.</span> : null}
      {write?.state === "rejected" ? (
        <RejectionView rejection={write.rejection} onRebase={() => { rename.keep(rebasedOnto(write.rejection, view.journey.header.id, view.journey.revision)); }} />
      ) : write === undefined ? null : (
        <WriteStatus view={write} />
      )}
    </div>
  );
}

/** B7: an orphaned node needs a choice: keep it as the journey's own, or remove it (in Edit structure, which shows what goes with it). */
function Orphaned({ view, detail, removeTo }: { view: Ready; detail: NodeDetail; removeTo: string }) {
  const write = useNodeWrite(view, `orphan:${detail.node.key}`);
  return (
    <div className="row" data-testid="orphaned">
      <span>Orphaned: its route version no longer has it.</span>
      {write.writable ? (
        <>
          <Button disabled={write.disabled} onClick={() => void write.run([{ op: "set_provenance", node: detail.node.key, provenance: "local" }])}>
            Keep
          </Button>
          <Link className="button" to={removeTo}>
            Remove…
          </Link>
        </>
      ) : null}
    </div>
  );
}

/** The owner, or an inline `Assign ▾` that lists the people. The date and the rank are said once, by the Dates section and the sentence under the title. */
function Meta({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const write = useNodeWrite(view, `assign:${detail.node.key}`);
  const { node, derived } = detail;
  const people = view.inputs.deployment.identities ?? [];
  const { caller } = useCaller();
  const owner =
    derived.unassigned === true && people.length > 0 && write.writable ? (
      <OwnerSelect key="assign" view={view} write={write} node={node.key} placeholder="Assign" className="assign-owner" testId="assign-owner" />
    ) : (
      ownerWords(view, ownersOf(view, node.key), caller?.identities)
    );
  return (
    <div className="meta muted" data-testid="detail-meta">
      {owner}
    </div>
  );
}

export function Header({ view, detail, rename, removeTo }: { view: Ready; detail: NodeDetail; rename: Rename; removeTo: string }) {
  const { node, record } = detail;
  const shown = detail.derived.display_state;
  // An unassigned node says so with the Assign control beside its meta line, not with a flag as well.
  const flags = flagsOf(detail.derived).filter(({ flag }) => flag !== "no owner");
  return (
    <div className="stack detail-head" data-testid="detail-header">
      <div className="row detail-band">
        <span className="detail-kind">
          <span className="label">{kindWord(node.kind, isStage(node))}</span>
          {detail.ancestors.map((ancestor) => (
            <span key={ancestor.key} className="muted small">
              {" › "}
              <NodeLink view={view} node={ancestor.key} />
            </span>
          ))}
        </span>
        <span className="spacer" />
        <Badge tone={statusTone(shown)} data-testid="detail-state" data-status={shown}>
          {statusWord(shown, node.kind)}
        </Badge>
      </div>
      {rename.open ? <RenameForm view={view} title={node.title} rename={rename} /> : <h2 data-testid="detail-title">{node.title}</h2>}
      <Receipt receipt={rename.receipt} />
      <Meta view={view} detail={detail} />
      {flags.length === 0 ? null : (
        <div className="row" data-testid="flags">
          {flags.map(({ flag, tone }) => (
            <Badge key={flag} tone={tone} data-testid="flag" data-status={flag}>
              {flag}
            </Badge>
          ))}
        </div>
      )}
      {record.provenance === "orphaned" ? <Orphaned view={view} detail={detail} removeTo={removeTo} /> : null}
    </div>
  );
}
