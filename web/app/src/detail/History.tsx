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
import { dateWords } from "../timeline/model.ts";
import { Markdown } from "../ui/markdown.tsx";
import { type AnswerValue, type GraphNode, type NodeDetail, type Ready } from "./model.ts";
import { listed, recount, wordsOf, type Happened, type Words } from "./happened.ts";
import { answerText } from "./sections.tsx";
import { Failed } from "../ui/Failed.tsx";

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

/** A page appended to what is shown: a patch split across pages is joined back together. */
export function appended(shown: PatchEvents[], page: HistoryPage): PatchEvents[] {
  const [first, ...rest] = page.patches;
  const last = shown.at(-1);
  if (first !== undefined && last?.patch_id === first.patch_id) {
    return [...shown.slice(0, -1), { ...last, events: [...last.events, ...first.events] }, ...rest];
  }
  return [...shown, ...page.patches];
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

/** Each patch with its events in words, the earlier events telling the later ones what they changed from. */
function withTold(patches: PatchEvents[], words: Words): { patch: PatchEvents; told: Happened[] }[] {
  const told = recount(
    patches.flatMap((patch) => patch.events),
    words,
  );
  let at = 0;
  return patches.map((patch) => {
    const mine = told.slice(at, at + patch.events.length);
    at += patch.events.length;
    return { patch, told: mine };
  });
}

const capitalized = (text: string): string => text.charAt(0).toUpperCase() + text.slice(1);

/** One patch: the date, then who did what in a sentence, the reason it was given with, and any answer it wrote. */
function PatchItem({ view, decision, patch, told, name }: { view: Ready; decision: GraphNode; patch: PatchEvents; told: Happened[]; name: (user: string) => string }) {
  const [first] = patch.events;
  const [lead] = told;
  if (first === undefined || lead === undefined) {
    return null;
  }
  const answers = answersWritten(patch, decision.key);
  const note = patch.events.find((event) => event.note != null)?.note;
  const what = listed([...new Set(told.map((each) => each.what))]);
  return (
    <li data-testid="history-patch" data-actor={lead.actor}>
      <span className="muted small">{dateWords(first.at.slice(0, 10), view.derived.today)}</span>{" "}
      <span data-testid="history-sentence">
        {name(lead.actor)} {what}
        {told.some((each) => each.byAgent) ? " (through an agent)" : ""}
      </span>
      {note == null ? null : <div className="muted small" data-testid="history-note">{note}</div>}
      {answers.map((answer, index) => (
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

/** J4: the node's history as sentences, read from the host while `open` and kept current as the journey moves. */
export function NodeHistory({ view, detail, open }: { view: Ready; detail: NodeDetail; open: boolean }) {
  const { host } = useSession();
  const journey = view.journey.header.id;
  const node = detail.node.key;
  const revision = view.journey.revision;
  const [loaded, setLoaded] = useState<Loaded | { failed: string } | undefined>();
  const reads = useRef(new Reads()).current;
  const name = useUserName();
  const words = wordsOf(view, node, name);
  const who = useUserWords();
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
          <ul className="detail-list">
            {withTold(loaded.patches, words).map(({ patch, told }) => (
              <PatchItem key={patch.patch_id} view={view} decision={detail.node} patch={patch} told={told} name={(user) => capitalized(who(user))} />
            ))}
          </ul>
          {loaded.next === undefined ? null : <Button onClick={() => { load(loaded.next, loaded.patches); }}>More</Button>}
        </>
      ) : null}
    </div>
  );
}
