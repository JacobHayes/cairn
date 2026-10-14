// The inspector when several nodes are selected: how many, who they are (title, kind, status, and
// a × to drop one; a click on the title keeps that one alone), then the actions over all of them
// (acting/SelectionActions.tsx) or, while the structure is edited, Remove. One member is not
// here: it is the node's own detail.
import { useEffect } from "react";
import { Link, useLocation } from "react-router";

import { RemoveSelected } from "../authoring/RemoveSelected.tsx";
import type { Authored } from "../authoring/target.ts";
import type { Selection } from "../canvas/selection.ts";
import { selectionOf } from "../acting/acts.ts";
import { NodePanel } from "../acting/NodeItem.tsx";
import { SelectionActions } from "../acting/SelectionActions.tsx";
import { statusTone, statusWord } from "../status/words.ts";
import { Badge, Button } from "../ui/kit.tsx";
import { titleOf, type Ready } from "./model.ts";
import { nodePath, screenPath } from "./parts.tsx";

export function SelectionPanel({ ready, selection, authored }: { ready: Ready; selection: Selection; authored: Authored | undefined }) {
  const { pathname, search } = useLocation();
  const members = selectionOf(ready, selection.members);
  // Removed nodes leave the journey and so the list; the panel stays up through a removal that goes node by node, and goes when none is left.
  const { clear } = selection;
  const empty = members.length === 0;
  useEffect(() => {
    if (empty) {
      clear();
    }
  }, [empty, clear]);
  return (
    <div className="stack" data-testid="selection-panel" data-count={members.length}>
      <div className="row">
        <strong>{members.length} selected</strong>
        <span className="spacer" />
        <Button ghost data-testid="selection-clear" onClick={selection.clear}>
          Clear
        </Button>
      </div>
      <NodePanel testId="selection-members">
        {members.map(({ node, derived }) => (
          <li key={node.key} className="node-row" data-testid="selection-member" data-node={node.key}>
            <div className="node-row-main">
              <Badge tone={statusTone(derived.display_state)}>{statusWord(derived.display_state, node.kind)}</Badge>
              <span className="node-row-text">
                <Link to={{ pathname: nodePath(screenPath(pathname), node.key), search }} className="node-title" data-node={node.key} onClick={selection.release}>
                  {titleOf(ready, node.key)}
                </Link>
                <span className="node-fact muted">{node.kind}</span>
              </span>
              <Button ghost aria-label={`Remove ${titleOf(ready, node.key)} from the selection`} title="Remove from the selection" onClick={() => { selection.toggle(node.key); }}>
                ×
              </Button>
            </div>
          </li>
        ))}
      </NodePanel>
      <SelectionActions
        view={ready}
        selected={members}
        onLanded={selection.clear}
        structure={authored === undefined ? undefined : <RemoveSelected authored={authored} keys={members.map((facts) => facts.node.key)} />}
      />
    </div>
  );
}
