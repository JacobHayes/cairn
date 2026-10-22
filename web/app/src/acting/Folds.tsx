// C10, B6, D3: the two folds under the list while Frontier is on, closed with their counts
// showing, each a bordered panel with a band header and the list's own row look, holding only
// what the journey's level draws (Detail, the Filter and the search). "Needs a look" holds the work that is wrong rather than next (stale, snoozed and overdue, short of its
// plan), each with its fix beside it. "Snoozed" holds the work set aside, grouped by the
// snooze that holds it: a container's, once for all it holds, or each node's own target. A
// node a container holds sits under the container only, even when it has a later snooze of its
// own, so each is listed once.
import type { ListQuery } from "@cairn/wasm";
import { useMemo, type ReactNode } from "react";
import { Link } from "react-router";

import { ActButton } from "../detail/ActButton.tsx";
import { newAttachmentKey } from "../detail/Attachments.tsx";
import { guardFailureText, namer } from "../detail/explain.ts";
import { itemTitle, titleOf, type Ready, type Mutation } from "../detail/model.ts";
import { Rejected } from "../detail/Rejected.tsx";
import { useFormDraft, useNodeWrite, type NodeWrite } from "../detail/write.ts";
import { snoozeEndWords } from "../status/snooze.ts";
import { daysUntil, daysWords } from "../status/when.ts";
import { upperFirst } from "../status/words.ts";
import { Button } from "../ui/kit.tsx";
import type { ListFlag } from "./address.ts";
import { NodeItem } from "./NodeItem.tsx";
import { useRows } from "./useRows.ts";
import type { NodeRow } from "./why.ts";

type SnoozeTarget = NonNullable<NodeRow["snoozed"]>;

function queryFor(flag: ListFlag): ListQuery {
  return { flags: [flag], sort: "rank" };
}

/** The rows with `flag` that the view draws (`drawn`, the journey's level: its depth, Filter and search), and how many there are. */
function useFlagged(view: Ready, flag: ListFlag, drawn: ReadonlySet<string> | undefined): { rows: NodeRow[]; total: number } {
  // Every page, so a drawn row past the first page is not lost to the level's filter.
  const query = useMemo(() => queryFor(flag), [flag]);
  const rows = (useRows(view, query).rows ?? []).filter((row) => drawn?.has(row.key) === true);
  return { rows, total: rows.length };
}

function Fold({ testId, title, count, children }: { testId: string; title: string; count: number; children: ReactNode }) {
  return (
    <details className="fold node-panel" data-testid={testId} data-count={count}>
      <summary className="node-panel-head">
        {title} <span className="fold-count">{count}</span>
      </summary>
      {children}
    </details>
  );
}

/** What a snooze waits for, in words. */
function untilWords(view: Ready, target: SnoozeTarget): string {
  return `until ${snoozeEndWords(view, target)}`;
}

/** Add the note that stale work is missing, in one patch. */
function AddNote({ write, node }: { write: NodeWrite; node: string }) {
  const form = useFormDraft<string>(write.journey, node, "stale-note");
  if (!write.writable) {
    return null;
  }
  if (form.draft === undefined) {
    return <Button onClick={() => { form.open("", write.seen); }}>Add note</Button>;
  }
  const text = form.draft.value;
  const note: Mutation = { op: "add_annotation", annotation: { key: newAttachmentKey(), node, note: text.trim() } };
  return (
    <span className="stack fold-form">
      <textarea aria-label="Note" placeholder="What was done, in a note" value={text} onChange={(event) => { form.change(event.target.value); }} />
      <span className="row">
        <Button
          primary
          disabled={write.disabled || text.trim() === ""}
          onClick={() => {
            void write.run([note], form.draft).then((landed) => {
              if (landed) {
                form.close();
              }
            });
          }}
        >
          Save note
        </Button>
        <Button onClick={() => { form.close(); write.dismiss(); }}>Cancel</Button>
      </span>
    </span>
  );
}

/** D4: a guard failure that makes finished work stale, as a sentence. */
function staleWords(view: Ready, key: string, failure: NonNullable<ReturnType<typeof firstStale>>): string {
  return upperFirst(guardFailureText(failure, namer(view), view.derived.today, (id) => itemTitle(view, key, id)));
}

function firstStale(view: Ready, key: string) {
  return view.derived.nodes[key]?.stale?.[0];
}

interface Look {
  row: NodeRow;
  reason: string;
  fix: "note" | "unsnooze" | "open";
}

/** One entry per node, with the reason that matters most: stale, then short of its plan, then snoozed and overdue. */
function looksOf(view: Ready, stale: NodeRow[], short: NodeRow[], lapsed: NodeRow[]): Look[] {
  const found = new Map<string, Look>();
  const add = (look: Look) => {
    if (!found.has(look.row.key)) {
      found.set(look.row.key, look);
    }
  };
  for (const row of stale) {
    const failure = firstStale(view, row.key);
    add({ row, reason: failure === undefined ? "Stale" : staleWords(view, row.key, failure), fix: failure === "missing_note" ? "note" : "open" });
  }
  for (const row of short) {
    add({ row, reason: `${daysWords(row.shortfall_days ?? 0)} short of its plan`, fix: "open" });
  }
  for (const row of lapsed) {
    const late = row.due == null ? undefined : -daysUntil(row.due, view.derived.today);
    add({ row, reason: late === undefined ? "Snoozed, and overdue" : `Snoozed, and ${daysWords(late)} late`, fix: "unsnooze" });
  }
  return [...found.values()];
}

/** The folds open no form: they offer only Unsnooze, which is one move. */
function noForm(): void {}

function LookRow({ view, write, look, to }: { view: Ready; write: NodeWrite; look: Look; to: string }) {
  return (
    <NodeItem view={view} node={look.row.key} to={to} fact={look.reason} testId="fold-item">
      <span className="row-side">
        {look.fix === "note" ? <AddNote write={write} node={look.row.key} /> : null}
        {look.fix === "unsnooze" ? <ActButton act="unsnooze" view={view} node={look.row.key} write={write} waiting={false} open={noForm} /> : null}
        {look.fix === "open" ? <Link className="button" to={to}>{look.reason.endsWith("plan") ? "Open dates" : "Open"}</Link> : null}
      </span>
    </NodeItem>
  );
}

/** One snooze and what it holds: a container's, or the nodes that share one target. */
interface Held {
  id: string;
  /** The container whose snooze it is, when it is a container's. */
  container: string | undefined;
  target: SnoozeTarget | undefined;
  rows: NodeRow[];
}

function heldBy(view: Ready, rows: NodeRow[]): Held[] {
  const groups = new Map<string, Held>();
  for (const row of rows) {
    const container = row.snoozed_via ?? undefined;
    const target = container === undefined ? (row.snoozed ?? undefined) : view.journey.graph.state?.snoozes?.[container];
    const id = container ?? JSON.stringify(target ?? null);
    const group = groups.get(id) ?? { id, container, target, rows: [] };
    group.rows.push(row);
    groups.set(id, group);
  }
  return [...groups.values()];
}

function SnoozedGroup({ view, write, held, toOf }: { view: Ready; write: NodeWrite; held: Held; toOf: (key: string) => string }) {
  const until = held.target === undefined ? "" : untilWords(view, held.target);
  return (
    <li className="stack fold-group" data-testid="snooze-group" data-holder={held.container ?? ""}>
      <span className="row fold-group-head">
        <strong>{held.container === undefined ? `Snoozed ${until}` : `${titleOf(view, held.container)} is snoozed ${until}`}</strong>
        {held.container === undefined ? null : (
          <ActButton act="unsnooze" view={view} node={held.container} write={write} waiting={false} open={noForm} />
        )}
      </span>
      <ul className="plain-list node-rows">
        {held.rows.map((row) => (
          <NodeItem key={row.key} view={view} node={row.key} to={toOf(row.key)} testId="fold-item">
            {held.container === undefined ? (
              <span className="row-side">
                <ActButton act="unsnooze" view={view} node={row.key} write={write} waiting={false} open={noForm} />
              </span>
            ) : null}
          </NodeItem>
        ))}
      </ul>
    </li>
  );
}

/** The folds under the list, each only while it holds something the view draws. */
export function Folds({ view, drawn, toOf }: { view: Ready; drawn: ReadonlySet<string> | undefined; toOf: (key: string) => string }) {
  const write = useNodeWrite(view, "folds");
  const stale = useFlagged(view, "stale", drawn);
  const short = useFlagged(view, "shortfall", drawn);
  const lapsed = useFlagged(view, "snoozed_and_overdue", drawn);
  const snoozed = useFlagged(view, "snoozed", drawn);
  const looks = looksOf(view, stale.rows, short.rows, lapsed.rows);
  const groups = heldBy(view, snoozed.rows);
  return (
    <div className="stack folds" data-testid="folds">
      {looks.length === 0 ? null : (
        <Fold testId="fold-look" title="Needs a look" count={looks.length}>
          <ul className="plain-list node-rows">
            {looks.map((look) => (
              <LookRow key={look.row.key} view={view} write={write} look={look} to={toOf(look.row.key)} />
            ))}
          </ul>
        </Fold>
      )}
      {snoozed.total === 0 ? null : (
        <Fold testId="fold-snoozed" title="Snoozed" count={snoozed.total}>
          <ul className="plain-list node-rows">
            {groups.map((held) => (
              <SnoozedGroup key={held.id} view={view} write={write} held={held} toOf={toOf} />
            ))}
          </ul>
        </Fold>
      )}
      <Rejected view={view} write={write} />
    </div>
  );
}
