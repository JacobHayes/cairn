// The row of a list of nodes that is not the table (Mine, the folds under the frontier, the pass
// rail): the table's look in a list, so every list of nodes reads the same. A status chip, the
// title with at most one fact under it, and what the row offers at its right end (`children`,
// its actions or a fix), on an opaque paper row with an edge hairline under it; `NodePanel` is
// the bordered panel the rows sit in, with an optional band header. Selecting the row (a click)
// opens the inspector for the rest.
import { useEffect, useRef, type MouseEvent, type ReactNode } from "react";
import { Link, useNavigate } from "react-router";

import { nodeOf, titleOf, type Ready } from "../detail/model.ts";
import { statusTone, statusWord } from "../status/words.ts";
import { Badge } from "../ui/kit.tsx";

/** A click on the row itself, not on a control inside it, selects it. */
function onRow(event: MouseEvent): boolean {
  return !(event.target instanceof Element && event.target.closest("a, button, select, input, textarea, label"));
}

export interface NodeItemProps {
  view: Ready;
  node: string;
  /** Where the inspector for this row opens. */
  to: string;
  selected?: boolean;
  /** The one fact the row says under its title, in words. */
  fact?: string | undefined;
  testId?: string;
  /** What the row offers at its right end, and the forms it opens under it. */
  children?: ReactNode;
}

export function NodeItem({ view, node, to, selected = false, fact, testId = "node-row", children }: NodeItemProps) {
  const navigate = useNavigate();
  const item = useRef<HTMLLIElement>(null);
  const state = view.derived.nodes[node]?.display_state;
  const kind = nodeOf(view, node)?.kind;
  // j and k move the selection: keep the selected row in view.
  useEffect(() => {
    if (selected) {
      item.current?.scrollIntoView({ block: "nearest" });
    }
  }, [selected]);
  return (
    <li
      ref={item}
      className="node-row"
      data-testid={testId}
      data-node={node}
      data-selected={selected}
      onClick={(event) => {
        if (onRow(event)) {
          void navigate(to);
        }
      }}
    >
      <div className="node-row-main">
        {state === undefined || kind === undefined ? null : <Badge tone={statusTone(state)}>{statusWord(state, kind)}</Badge>}
        <span className="node-row-text">
          <Link to={to} className="node-title" data-node={node} aria-current={selected ? "true" : undefined}>
            {titleOf(view, node)}
          </Link>
          {fact === undefined ? null : (
            <span className="node-fact muted" data-testid="fact">
              {fact}
            </span>
          )}
        </span>
      </div>
      {children}
    </li>
  );
}

/** The bordered panel a list of rows sits in, with a band header naming it when it has a `title`. */
export function NodePanel({ title, count, testId, children }: { title?: string; count?: number; testId?: string; children: ReactNode }) {
  return (
    <section className="node-panel" aria-label={title} {...(testId === undefined ? {} : { "data-testid": testId })}>
      {title === undefined ? null : (
        <header className="node-panel-head">
          {title}
          {count === undefined ? null : <span className="fold-count">{count}</span>}
        </header>
      )}
      <ul className="plain-list node-rows">{children}</ul>
    </section>
  );
}
