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
  fact?: ReactNode;
  /** The ink a date or trouble fact is said in, beside its words: warning or error. */
  factTone?: "warn" | "bad" | undefined;
  testId?: string;
  /** Another class for the row, for a list that styles its own. */
  className?: string;
  /** What the row offers at its right end, and the forms it opens under it. */
  children?: ReactNode;
}

export function NodeItem({ view, node, to, selected = false, fact, factTone, testId = "node-row", className, children }: NodeItemProps) {
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
      className={className === undefined ? "node-row" : `node-row ${className}`}
      data-testid={testId}
      data-row
      data-node={node}
      data-state={state}
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
          <Link to={to} className="node-title" data-row-title data-node={node} aria-current={selected ? "true" : undefined}>
            {titleOf(view, node)}
          </Link>
          {fact === undefined ? null : (
            <span className={factTone === undefined ? "node-fact muted" : `node-fact node-fact-${factTone}`} data-testid="fact" {...(typeof fact === "string" ? { title: fact } : {})}>
              {fact}
            </span>
          )}
        </span>
      </div>
      {children}
    </li>
  );
}

/**
 * The bordered panel a list of rows sits in, with a band header naming it when it has a `title`
 * (a real heading; a string, or a link to the thing the rows belong to, then `label` names the panel),
 * its `count` (or, in its place, an `aside` at the head's right end that names it), and a foot for what acts on the whole list.
 */
export function NodePanel({
  title,
  label,
  count,
  aside,
  testId,
  foot,
  children,
}: {
  title?: ReactNode;
  label?: string;
  count?: number;
  aside?: ReactNode;
  testId?: string;
  foot?: ReactNode;
  children: ReactNode;
}) {
  const name = label ?? (typeof title === "string" ? title : undefined);
  return (
    <section className="node-panel" {...(name === undefined ? {} : { "aria-label": name })} {...(testId === undefined ? {} : { "data-testid": testId })}>
      {title === undefined ? null : (
        <header className="node-panel-head">
          <h2 className="node-panel-title">{title}</h2>
          {aside !== undefined ? <span className="node-panel-aside">{aside}</span> : count === undefined ? null : <span className="fold-count">{count}</span>}
        </header>
      )}
      <ul className="plain-list node-rows">{children}</ul>
      {foot === undefined ? null : <footer className="node-panel-foot">{foot}</footer>}
    </section>
  );
}
