// C8, D8: the inspector's header (design 6.2): the kind and where the node sits (each crumb a
// link) with its display state at the top right, its title (renamed in place from the menu),
// one meta line (the owner, the one date that matters, the rank when it is on the acting
// frontier), the flags that are set, and, for an orphaned node, the choice it needs. Where the
// node came from is not here: it is the Origin section.
import { useEffect, useState } from "react";
import { Link } from "react-router";

import { assignOwner } from "../acting/acts.ts";
import { useDraft } from "../data/drafts.ts";
import { unlandedOf, useProblem, useSession, useSkew } from "../data/react.ts";
import type { Rejection, WriteResult } from "../data/writes.ts";
import { Menu } from "../screens/Menu.tsx";
import { RejectionView } from "../screens/RejectionView.tsx";
import { statusTone, statusWord } from "../status/words.ts";
import { Badge, Button, Field } from "../ui/kit.tsx";
import { flagsOf, isTerminal, type NodeDetail, type Ready } from "./model.ts";
import { NodeLink } from "./parts.tsx";
import { entityName } from "./sections.tsx";
import { ownerName } from "./sentence.ts";
import { dueLine } from "./words.ts";
import { rebasedOnto, useNodeWrite } from "./write.ts";

interface TitleDraft {
  text: string;
  /** The journey revision the author saw when they started editing. */
  base: number;
}

/** What renaming the open node needs: its draft, kept across a reload, and the way to start it. */
export interface Rename {
  draft: TitleDraft | undefined;
  start: () => void;
  change: (text: string) => void;
  /** Keeps the text and drafts it against `base` instead, after a conflict. */
  rebase: (base: number) => void;
  cancel: () => void;
}

/** The node's rename: a draft (its text and the revision its author saw) in session storage, so a reload keeps it. */
export function useRename(view: Ready, node: string, title: string): Rename {
  const journey = view.journey.header.id;
  const [draft, setDraft] = useDraft<TitleDraft>(`title:${journey}:${node}`);
  return {
    draft,
    start: () => {
      setDraft({ text: title, base: view.journey.revision });
    },
    change: (text) => {
      if (draft !== undefined) {
        setDraft({ ...draft, text });
      }
    },
    rebase: (base) => {
      if (draft !== undefined) {
        setDraft({ ...draft, base });
      }
    },
    cancel: () => {
      setDraft(undefined);
    },
  };
}

/** The title as an input: saving sends one `set_node_field` against the revision it was drafted on (H5). */
function RenameForm({ view, node, title, rename }: { view: Ready; node: string; title: string; rename: Rename }) {
  const session = useSession();
  const skew = useSkew();
  const [rejection, setRejection] = useState<Rejection | undefined>();
  const [saving, setSaving] = useState(false);
  const { draft } = rename;
  const journey = view.journey.header.id;
  // A rejected rename stays on the sync chip while it is shown here: a conflict, or NOT SAVED.
  const key = `title:${journey}:${node}`;
  useProblem(key, rejection === undefined ? undefined : unlandedOf(rejection), { label: title, discard: () => { setRejection(undefined); } });
  useEffect(() => () => { session.sync.resolve(key); }, [session, key]);
  if (draft === undefined) {
    return null;
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
      rename.cancel();
    }
  };
  return (
    <div className="stack" data-testid="rename">
      <span className="row">
        <Field
          aria-label={`New title for ${title}`}
          value={draft.text}
          disabled={saving}
          onChange={(event) => { rename.change(event.target.value); }}
          onKeyDown={(event) => {
            if (event.key === "Enter" && draft.text.trim() !== "") {
              void save();
            }
          }}
        />
        <Button primary disabled={saving || skew !== undefined || draft.text.trim() === ""} onClick={() => void save()}>
          Save
        </Button>
        <Button onClick={() => { rename.cancel(); setRejection(undefined); }}>Cancel</Button>
      </span>
      {draft.base < view.journey.revision ? <span className="muted small">Editing as of revision {draft.base}; now {view.journey.revision}.</span> : null}
      {rejection === undefined ? null : (
        <RejectionView
          rejection={rejection}
          onRebase={() => {
            rename.rebase(rebasedOnto(rejection, journey, view.journey.revision));
            setRejection(undefined);
          }}
        />
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
      <Button disabled={write.disabled} onClick={() => void write.run([{ op: "set_provenance", node: detail.node.key, provenance: "local" }])}>
        Keep
      </Button>
      <Link className="button" to={removeTo}>
        Remove…
      </Link>
    </div>
  );
}

/** The owner, or an inline `Assign ▾` that lists the people; the date in words; the rank when it is on the acting frontier. */
function Meta({ view, detail, position }: { view: Ready; detail: NodeDetail; position: number | undefined }) {
  const write = useNodeWrite(view, `assign:${detail.node.key}`);
  const { node, derived, record } = detail;
  const open = !isTerminal(record.state) && derived.relevance.value !== "not_relevant";
  const people = view.inputs.deployment.entities ?? [];
  const owner =
    derived.unassigned === true && people.length > 0 ? (
      <Menu key="assign" label="Assign owner" testId="assign-owner" trigger="Assign ▾">
        {(close) =>
          people.map((each) => (
            <button
              key={each.key}
              type="button"
              role="menuitem"
              className="menu-item"
              disabled={write.disabled}
              onClick={() => {
                close();
                void write.run([assignOwner(node.key, each.key)]);
              }}
            >
              {entityName(view, each.key)}
            </button>
          ))
        }
      </Menu>
    ) : (
      ownerName(view, node.key)
    );
  const date = open ? dueLine(node.kind, derived.dates.due?.date, view.derived.today) : undefined;
  const parts = [owner, date, position === undefined ? undefined : `Rank #${String(position)}`].filter((part) => part !== undefined);
  return (
    <div className="meta muted" data-testid="detail-meta">
      {parts.flatMap((part, at) => (at === 0 ? [part] : [" · ", part]))}
    </div>
  );
}

export function Header({ view, detail, position, rename, removeTo }: { view: Ready; detail: NodeDetail; position: number | undefined; rename: Rename; removeTo: string }) {
  const { node, record } = detail;
  const shown = detail.derived.display_state;
  // An unassigned node says so with the Assign control beside its meta line, not with a flag as well.
  const flags = flagsOf(detail.derived, record.state).filter(({ flag }) => flag !== "unassigned");
  return (
    <div className="stack detail-head" data-testid="detail-header">
      <div className="row detail-band">
        <span className="detail-kind">
          <span className="label">{node.kind}</span>
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
      {rename.draft === undefined ? <h2 data-testid="detail-title">{node.title}</h2> : <RenameForm view={view} node={node.key} title={node.title} rename={rename} />}
      <Meta view={view} detail={detail} position={position} />
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
