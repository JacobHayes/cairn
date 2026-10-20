// The Summary's rows: an `Entry` is a node of one journey with its one fact and what it offers, and
// `EntriesPanel` draws a list of them as the shared bounded panel (acting/RowsPanel.tsx).
import type { ReactNode } from "react";

import { NodeItem } from "../acting/NodeItem.tsx";
import type { FactTone } from "../acting/reasons.ts";
import { RowActions } from "../acting/RowActions.tsx";
import { RowsPanel } from "../acting/RowsPanel.tsx";
import type { NodeRow } from "../acting/why.ts";
import type { Ready } from "../detail/model.ts";
import { nodeAddress } from "./links.ts";
import { useViewer } from "../data/react.ts";
import { ownerWords } from "../detail/sections.tsx";

/** The owner as a row says it when it offers no actions: who has it, or that nobody does. */
function OwnerText({ view, owners }: { view: Ready; owners: string[] | undefined }) {
  const { viewer } = useViewer();
  if (owners === undefined) {
    return null;
  }
  const words = ownerWords(view, owners, viewer?.entities);
  return (
    <div className="row-side">
      <span className="row-owner row-owner-static" title={owners.length === 0 ? undefined : `Owned by ${words}`}>
        {words}
      </span>
    </div>
  );
}

/** One row of a list: a node with its one fact, and what it offers when `live`. */
export interface Entry {
  key: string;
  /** What it says under the title; a function is read only when the row is drawn (a long list reads five). */
  fact: ReactNode | (() => ReactNode);
  tone: FactTone | undefined;
  /** The row's actions, for a row that has them; read-only rows give its owner's names. */
  act: NodeRow | undefined;
  /** The owners' keys, none when the row says nothing of who has it (finished work). */
  owners: string[] | undefined;
  /** The row's work is the viewer's (C18: the acting-frontier items they own). */
  yours?: boolean;
}

interface Props {
  ready: Ready;
  title: string;
  testId: string;
  entries: Entry[];
  /** Whether rows that can act offer their actions (not on an archived journey). */
  actions: boolean;
  owner?: boolean;
  /** What acts on the whole list, at its foot beside Show all. */
  foot?: ReactNode;
  folded?: boolean;
  open?: boolean;
  bright?: boolean;
}

/** A list of the journey's nodes in a bounded panel: each row opens the node's own address and, shown, offers its actions. */
export function EntriesPanel({ ready, entries, actions, owner = true, ...panel }: Props) {
  const journey = ready.journey.header.id;
  const rows = entries.map((entry) => ({
    key: `${journey}/${entry.key}`,
    render: (shown: boolean) => {
      const to = nodeAddress(journey, entry.key);
      return (
        <NodeItem view={ready} node={entry.key} to={to} fact={typeof entry.fact === "function" ? entry.fact() : entry.fact} factTone={entry.tone} testId="summary-item">
          {actions && entry.act !== undefined && shown ? <RowActions view={ready} row={entry.act} to={to} owner={owner} yours={entry.yours === true} /> : <OwnerText view={ready} owners={entry.owners} />}
        </NodeItem>
      );
    },
  }));
  return <RowsPanel {...panel} rows={rows} />;
}
