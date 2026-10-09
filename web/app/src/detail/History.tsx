// C8, J4: the node's history, the events that wrote anything on it, grouped by patch, newest
// page last, read from the host on request (events are not in the document) and paged with
// "more". An open history reads its first page again when the journey moves, so it stays
// current (H6) without fetching anything while folded.
import type { Schema } from "@cairn/client";
import type { HistoryPage } from "@cairn/wasm";
import { useEffect, useRef, useState } from "react";

import type { Host } from "../data/host.ts";
import { useSession } from "../data/react.ts";
import { Button } from "../ui/kit.tsx";
import { Markdown } from "../ui/markdown.tsx";
import type { AnswerValue, GraphNode, NodeDetail, Ready } from "./model.ts";
import { answerText } from "./sections.tsx";

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

function PatchItem({ view, decision, patch }: { view: Ready; decision: GraphNode; patch: PatchEvents }) {
  const [first] = patch.events;
  if (first === undefined) {
    return null;
  }
  const answers = answersWritten(patch, decision.key);
  const note = patch.events.find((event) => event.note != null)?.note;
  return (
    <li data-testid="history-patch">
      <span className="muted small">
        {first.at} by {first.actor.user}
        {first.actor.agent == null ? "" : ` (agent ${first.actor.agent})`}
      </span>{" "}
      {[...new Set(patch.events.map((event) => event.event_type.replaceAll("_", " ")))].join(", ")}
      {note == null ? null : <span className="muted small"> — {note}</span>}
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

export function NodeHistory({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const { host } = useSession();
  const journey = view.journey.header.id;
  const node = detail.node.key;
  const revision = view.journey.revision;
  const [open, setOpen] = useState(false);
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
    <details className="detail-section" data-testid="history" onToggle={(event) => { setOpen(event.currentTarget.open); }}>
      <summary>
        <span className="detail-section-title">History</span>
      </summary>
      <div className="stack detail-section-body">
        {loaded === undefined ? <span className="muted small">Reading...</span> : null}
        {loaded !== undefined && "failed" in loaded ? <span className="callout callout-bad">The history could not be read: {loaded.failed}</span> : null}
        {loaded !== undefined && "patches" in loaded ? (
          <>
            <ol className="detail-list">
              {loaded.patches.map((patch) => (
                <PatchItem key={patch.patch_id} view={view} decision={detail.node} patch={patch} />
              ))}
            </ol>
            {loaded.next === undefined ? null : (
              <Button onClick={() => { load(loaded.next, loaded.patches); }}>More</Button>
            )}
          </>
        ) : null}
      </div>
    </details>
  );
}
