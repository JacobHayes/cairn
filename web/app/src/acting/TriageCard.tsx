// C11: one triage card, the focus of the pass: the node's kind, title, breadcrumb, why it ranks
// where it does, a decision's prompt, its description, and its kind's actions with pass.
import { useEffect } from "react";

import { nodeOf, type Ready } from "../detail/model.ts";
import { statusTone, statusWord } from "../status/words.ts";
import { Badge } from "../ui/kit.tsx";
import { Markdown } from "../ui/markdown.tsx";
import { factsOf } from "./acts.ts";
import { Acts } from "./Acts.tsx";
import { Crumb, DetailLink, Flags, Why } from "./Parts.tsx";
import type { NodeRow } from "./why.ts";

/** Whether a key press belongs to a field being typed in, not to the card. */
function typing(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && (target.isContentEditable || ["INPUT", "SELECT", "TEXTAREA"].includes(target.tagName));
}

/** The card's keyboard: P passes, unless a field has the keys or a modifier is held. */
function usePassKey(onPass: () => void): void {
  useEffect(() => {
    const heard = (event: KeyboardEvent) => {
      if (event.key.toLowerCase() === "p" && !event.ctrlKey && !event.metaKey && !event.altKey && !typing(event.target)) {
        event.preventDefault();
        onPass();
      }
    };
    document.addEventListener("keydown", heard);
    return () => {
      document.removeEventListener("keydown", heard);
    };
  }, [onPass]);
}

export function TriageCard({ view, row, position, total, onPass }: { view: Ready; row: NodeRow; position: number; total: number; onPass: () => void }) {
  usePassKey(onPass);
  const node = nodeOf(view, row.key);
  const facts = factsOf(view, row.key);
  if (node === undefined || facts === undefined) {
    return null;
  }
  return (
    <article className="panel stack triage-card" aria-label={node.title} data-testid="triage-card" data-node={row.key} data-kind={row.kind}>
      <div className="row">
        <span className="muted" data-testid="card-position">
          Card {position} of {total}
        </span>
        <span className="shell-spacer" />
        <span className="muted">P passes</span>
      </div>
      <Crumb view={view} row={row} />
      <div className="row">
        <h2 className="title">
          <DetailLink view={view} node={row.key} />
        </h2>
        <Badge>{row.kind}</Badge>
        <Badge tone={statusTone(row.display_state)}>{statusWord(row.display_state, row.kind)}</Badge>
        <Flags view={view} row={row} />
      </div>
      {node.kind === "decision" && node.prompt !== undefined ? <Markdown text={node.prompt} data-testid="prompt" /> : null}
      {node.description == null || node.description === "" ? null : <Markdown text={node.description} />}
      <span className="muted">
        {row.due == null ? "No deadline" : `Due ${row.due}`}
        {row.slack_days == null ? "" : `; slack ${String(row.slack_days)} days`}
      </span>
      <Why view={view} row={row} sort="rank" />
      <Acts view={view} facts={facts} onPass={onPass} />
    </article>
  );
}
