// The inspector when several nodes are selected: how many, who they are (title, kind, status, and
// a × to drop one; a click on the title keeps that one alone), then the actions over all of them
// (acting/SelectionActions.tsx) or, while the structure is edited, Remove. One member is not
// here: it is the node's own detail.
import { useEffect } from "react";

import { RemoveSelected } from "../authoring/RemoveSelected.tsx";
import type { Authored } from "../authoring/target.ts";
import type { Selection } from "../canvas/selection.ts";
import { selectionOf } from "../acting/acts.ts";
import { NodeItem, NodePanel } from "../acting/NodeItem.tsx";
import { SelectionActions } from "../acting/SelectionActions.tsx";
import { useNodeAddresses } from "../journeys/address.ts";
import { Button } from "../ui/kit.tsx";
import { isStage, kindWord, type Ready, titleOf } from "./model.ts";

export function SelectionPanel({ ready, selection, authored }: { ready: Ready; selection: Selection; authored: Authored | undefined }) {
  const addresses = useNodeAddresses();
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
        {members.map(({ node }) => (
          <NodeItem key={node.key} view={ready} node={node.key} to={addresses.open(node.key)} fact={kindWord(node.kind, isStage(node))} testId="selection-member" onOpen={selection.release}>
            <div className="row-side">
              <Button ghost aria-label={`Remove ${titleOf(ready, node.key)} from the selection`} title="Remove from the selection" onClick={() => { selection.toggle(node.key); }}>
                ×
              </Button>
            </div>
          </NodeItem>
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
