// C10, B6, D3: the two folds under the next list, closed with their counts showing. "Needs a
// look" holds the work that is wrong rather than next (stale, snoozed and overdue, short of its
// plan), each with its fix beside it. "Snoozed" holds the work set aside, grouped by the
// snooze that holds it: a container's, once for all it holds, or each node's own target. A
// node a container holds sits under the container only, even when it has a later snooze of its
// own, so each is listed once.
import type { ListQuery } from "@cairn/wasm";
import { useMemo, type ReactNode } from "react";
import { Link } from "react-router";

import { useProjected } from "../canvas/hooks.ts";
import { newAttachmentKey } from "../detail/Attachments.tsx";
import { namer } from "../detail/explain.ts";
import { titleOf, type Ready, type Mutation } from "../detail/model.ts";
import { Rejected } from "../detail/Rejected.tsx";
import { useFormDraft, useNodeWrite, type NodeWrite } from "../detail/write.ts";
import { dateWords, dayOf } from "../timeline/model.ts";
import { Button } from "../ui/kit.tsx";
import type { ListFlag, NextSettings } from "./address.ts";
import type { NodeRow } from "./why.ts";

type SnoozeTarget = NonNullable<NodeRow["snoozed"]>;

function queryFor(flag: ListFlag, settings: NextSettings): ListQuery {
  return { flags: settings.mine ? ["mine", flag] : [flag], states: [], kinds: settings.decisions ? ["decision"] : settings.kinds, sort: "rank" };
}

/** The rows with `flag`, narrowed as the next list is (not-relevant nodes never appear), and how many there are in all. */
function useFlagged(view: Ready, flag: ListFlag, settings: NextSettings): { rows: NodeRow[]; total: number } {
  const request = useMemo(() => ({ projection: "list" as const, query: queryFor(flag, settings) }), [flag, settings]);
  const { value } = useProjected(view, request);
  const wanted = settings.text.trim().toLowerCase();
  const live = (value?.rows ?? []).filter((row) => row.relevance !== "not_relevant");
  const rows = live.filter((row) => wanted === "" || row.title.toLowerCase().includes(wanted));
  const dropped = (value?.rows.length ?? 0) - live.length;
  return { rows, total: wanted === "" ? Math.max((value?.total ?? 0) - dropped, 0) : rows.length };
}

function Fold({ testId, title, count, children }: { testId: string; title: string; count: number; children: ReactNode }) {
  return (
    <details className="fold" data-testid={testId} data-count={count}>
      <summary>
        {title} <span className="fold-count">{count}</span>
      </summary>
      <div className="stack fold-body">{children}</div>
    </details>
  );
}

/** What a snooze waits for, in words. */
function untilWords(view: Ready, target: SnoozeTarget): string {
  return "date" in target ? `until ${dateWords(target.date, view.derived.today)}` : `until ${titleOf(view, target.node)} is done`;
}

const days = (count: number) => `${String(count)} ${count === 1 ? "day" : "days"}`;

/** Add the note that stale work is missing, in one patch. */
function AddNote({ write, node }: { write: NodeWrite; node: string }) {
  const form = useFormDraft<string>(write.journey, node, "stale-note");
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
function staleWords(view: Ready, failure: NonNullable<ReturnType<typeof firstStale>>): string {
  if (failure === "missing_note") {
    return "Needs a note";
  }
  if (failure === "missing_artifact") {
    return "Needs a link";
  }
  if (failure === "not_broken_down") {
    return "Was not broken down";
  }
  return `${namer(view)(failure.open_dependency)} was reopened`;
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
    add({ row, reason: failure === undefined ? "Stale" : staleWords(view, failure), fix: failure === "missing_note" ? "note" : "open" });
  }
  for (const row of short) {
    add({ row, reason: `${days(row.shortfall_days ?? 0)} short of its plan`, fix: "open" });
  }
  for (const row of lapsed) {
    const late = row.due == null ? undefined : dayOf(view.derived.today) - dayOf(row.due);
    add({ row, reason: late === undefined ? "Snoozed, and overdue" : `Snoozed, and ${days(late)} late`, fix: "unsnooze" });
  }
  return [...found.values()];
}

function LookRow({ view, write, look, to }: { view: Ready; write: NodeWrite; look: Look; to: string }) {
  const holder = look.row.snoozed_via ?? look.row.key;
  return (
    <li className="fold-row" data-testid="fold-item" data-node={look.row.key} data-reason={look.fix}>
      <span className="fold-row-text">
        <Link to={to} className="next-title" data-node={look.row.key}>
          {look.row.title}
        </Link>
        <span className="next-fact muted">{look.reason}</span>
      </span>
      {look.fix === "note" ? <AddNote write={write} node={look.row.key} /> : null}
      {look.fix === "unsnooze" ? (
        <Button disabled={write.disabled} onClick={() => void write.run([{ op: "unsnooze", node: holder }])}>
          {holder === look.row.key ? "Unsnooze" : `Unsnooze ${titleOf(view, holder)}`}
        </Button>
      ) : null}
      {look.fix === "open" ? <Link className="button" to={to}>{look.reason.endsWith("plan") ? "Open dates" : "Open"}</Link> : null}
    </li>
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
          <Button disabled={write.disabled} onClick={() => void write.run([{ op: "unsnooze", node: held.container ?? "" }])}>
            Unsnooze
          </Button>
        )}
      </span>
      <ul className="plain-list">
        {held.rows.map((row) => (
          <li key={row.key} className="fold-row" data-testid="fold-item" data-node={row.key}>
            <Link to={toOf(row.key)} className="next-title" data-node={row.key}>
              {row.title}
            </Link>
            {held.container === undefined ? (
              <Button disabled={write.disabled} onClick={() => void write.run([{ op: "unsnooze", node: row.key }])}>
                Unsnooze
              </Button>
            ) : null}
          </li>
        ))}
      </ul>
    </li>
  );
}

/** The folds under the list, each only while it holds something. */
export function Folds({ view, settings, toOf }: { view: Ready; settings: NextSettings; toOf: (key: string) => string }) {
  const write = useNodeWrite(view, "folds");
  const stale = useFlagged(view, "stale", settings);
  const short = useFlagged(view, "shortfall", settings);
  const lapsed = useFlagged(view, "snoozed_and_overdue", settings);
  const snoozed = useFlagged(view, "snoozed", settings);
  const looks = looksOf(view, stale.rows, short.rows, lapsed.rows);
  const groups = heldBy(view, snoozed.rows);
  return (
    <div className="stack folds" data-testid="folds">
      {looks.length === 0 ? null : (
        <Fold testId="fold-look" title="Needs a look" count={looks.length}>
          <ul className="plain-list fold-list">
            {looks.map((look) => (
              <LookRow key={look.row.key} view={view} write={write} look={look} to={toOf(look.row.key)} />
            ))}
          </ul>
        </Fold>
      )}
      {snoozed.total === 0 ? null : (
        <Fold testId="fold-snoozed" title="Snoozed" count={snoozed.total}>
          <ul className="plain-list fold-list">
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
