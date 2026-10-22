// C8, J4: the node's history, the events that wrote anything on it, grouped by patch, newest
// page last, read from the host on request (events are not in the document) and paged with
// "more". An open history reads its first page again when the journey moves, so it stays
// current (H6) without fetching anything while folded.
import type { Schema } from "@cairn/client";
import type { HistoryPage } from "@cairn/wasm";
import { useEffect, useRef, useState } from "react";

import type { Host } from "../data/host.ts";
import { useSession, useUserName, useUserWords } from "../data/react.ts";
import { Button } from "../ui/kit.tsx";
import { dateWords } from "../status/when.ts";
import { Markdown } from "../ui/markdown.tsx";
import { type AnswerValue, type GraphNode, type NodeDetail, type Ready } from "./model.ts";
import { foldTicks, recount, wordsOf, type Happened } from "./happened.ts";
import { answerText } from "./sections.tsx";
import { Failed } from "../ui/Failed.tsx";
import { namedList } from "../ui/words.ts";

type PatchEvents = Schema<"PatchEvents">;

/** An answer an event wrote on a node, with the reason it was given with, if any (B2, J1). */
export interface AnswerEntry {
  value: AnswerValue;
  rationale: string | undefined;
}

/** J1: the answers `patch`'s events wrote on `node`, each with its own rationale. */
export function answersWritten(patch: PatchEvents, node: string): AnswerEntry[] {
  return patch.events.flatMap((event) =>
    event.delta.flatMap((write) => {
      if (!("put" in write) || !("graph" in write.put)) {
        return [];
      }
      const record = write.put.graph.record;
      return "answer" in record && record.answer.decision === node ? [{ value: record.answer.value, rationale: record.answer.rationale ?? undefined }] : [];
    }),
  );
}

interface Loaded {
  patches: PatchEvents[];
  next: number | undefined;
}

/** Patches with a patch split across two pages joined back into one. */
export function joinPatches(patches: PatchEvents[]): PatchEvents[] {
  const whole: PatchEvents[] = [];
  for (const patch of patches) {
    const last = whole.at(-1);
    if (last?.patch_id === patch.patch_id) {
      whole[whole.length - 1] = { ...last, events: [...last.events, ...patch.events] };
    } else {
      whole.push(patch);
    }
  }
  return whole;
}

/** A page appended to what is shown. */
export function appended(shown: PatchEvents[], page: HistoryPage): PatchEvents[] {
  return joinPatches([...shown, ...page.patches]);
}

/**
 * Numbers reads so only the latest one's answer is shown: a read started before the journey
 * moved (or before a newer page was asked for) may answer after the one started since.
 */
export class Reads {
  #latest = 0;

  /** Starts a read: its ticket. */
  start(): number {
    this.#latest += 1;
    return this.#latest;
  }

  /** Whether `ticket` is the latest read started. */
  current(ticket: number): boolean {
    return ticket === this.#latest;
  }
}

/** Reads a page of the node's history after `after` and shows it after what is shown, if still wanted. */
function readPage(
  host: Host,
  reads: Reads,
  page: { journey: string; node: string; after: number | undefined; shown: PatchEvents[] },
  show: (loaded: Loaded | { failed: string }) => void,
): void {
  const ticket = reads.start();
  host.history(page.journey, page.node, page.after).then(
    (read) => {
      if (reads.current(ticket)) {
        show({ patches: appended(page.shown, read), next: read.next ?? undefined });
      }
    },
    (thrown: unknown) => {
      if (reads.current(ticket)) {
        show({ failed: thrown instanceof Error ? thrown.message : String(thrown) });
      }
    },
  );
}

const capitalized = (text: string): string => text.charAt(0).toUpperCase() + text.slice(1);

/** What a list of patches is the history of: the journey as it stands (when ready), the node it is of (none for a journey's own), and the decision whose answers each patch's lines show. */
interface PatchScope {
  view: Ready | undefined;
  node: string;
  decision?: GraphNode | undefined;
  today: string;
}

/** One patch: the date, then who did what in a sentence, the reason it was given with, and any answer it wrote. */
function PatchItem({ scope, patch, told, name }: { scope: PatchScope; patch: PatchEvents; told: Happened[]; name: (user: string) => string }) {
  const [first] = patch.events;
  const [lead] = told;
  if (first === undefined || lead === undefined) {
    return null;
  }
  const { view, decision } = scope;
  const answers = decision === undefined ? [] : answersWritten(patch, decision.key);
  const note = patch.events.find((event) => event.note != null)?.note;
  const what = namedList([...new Set(foldTicks(told).map((each) => each.what))], Infinity);
  return (
    <li data-testid="history-patch" data-actor={lead.actor}>
      <span className="muted small">{dateWords(first.at.slice(0, 10), scope.today)}</span>{" "}
      <span data-testid="history-sentence">
        {name(lead.actor)} {what}
        {told.some((each) => each.byAgent) ? " (through an agent)" : ""}
      </span>
      {note == null ? null : <div className="muted small" data-testid="history-note">{note}</div>}
      {view === undefined || decision === undefined
        ? null
        : answers.map((answer, index) => (
            <div key={index} className="stack" data-testid="history-answer">
              <span>
                Answer: <strong>{answerText(view, answer.value, decision)}</strong>
              </span>
              {answer.rationale === undefined ? <span className="muted small">No reason given.</span> : <Markdown text={answer.rationale} data-testid="history-rationale" />}
            </div>
          ))}
    </li>
  );
}

/**
 * The patches as sentences, one list item each: who did what, in order, each event telling
 * the later ones what it changed from. The one place a patch's changes are put in words (a
 * node's history, and what moved under a proposal).
 */
export function PatchList({ scope, patches, testId }: { scope: PatchScope; patches: PatchEvents[]; testId?: string }) {
  const name = useUserName();
  const who = useUserWords();
  const told = recount(
    patches.flatMap((patch) => patch.events),
    wordsOf(scope.view, scope.node, name),
  );
  let at = 0;
  return (
    <ul className="detail-list" data-testid={testId}>
      {patches.map((patch) => {
        const mine = told.slice(at, at + patch.events.length);
        at += patch.events.length;
        return <PatchItem key={patch.patch_id} scope={scope} patch={patch} told={mine} name={(user) => capitalized(who(user))} />;
      })}
    </ul>
  );
}

/** J4: the node's history as sentences, read from the host while `open` and kept current as the journey moves. */
export function NodeHistory({ view, detail, open }: { view: Ready; detail: NodeDetail; open: boolean }) {
  const { host } = useSession();
  const journey = view.journey.header.id;
  const node = detail.node.key;
  const revision = view.journey.revision;
  const [loaded, setLoaded] = useState<Loaded | { failed: string } | undefined>();
  const reads = useRef(new Reads()).current;
  const load = (after: number | undefined, shown: PatchEvents[]) => {
    readPage(host, reads, { journey, node, after, shown }, setLoaded);
  };
  // The first page again whenever the journey moves while the history is open.
  useEffect(() => {
    if (open) {
      readPage(host, reads, { journey, node, after: undefined, shown: [] }, setLoaded);
    }
  }, [host, reads, open, journey, node, revision]);
  return (
    <div className="stack" data-testid="history">
      <strong className="detail-subhead">History</strong>
      {loaded === undefined ? <span className="muted small">Reading...</span> : null}
      {loaded !== undefined && "failed" in loaded ? <span className="callout callout-bad"><Failed what="The history" error={loaded.failed} /></span> : null}
      {loaded !== undefined && "patches" in loaded ? (
        <>
          <PatchList scope={{ view, node, decision: detail.node, today: view.derived.today }} patches={loaded.patches} />
          {loaded.next === undefined ? null : <Button onClick={() => { load(loaded.next, loaded.patches); }}>More</Button>}
        </>
      ) : null}
    </div>
  );
}
