// A route's draft authored by hand on its canvas (A11, A12): the draft is always in edit mode,
// with the palette and the roles and participation kinds (StructureTools.tsx), drawing a requirement between two
// cards, and a node's form in the inspector at the draft's address with the node's key. A
// published version is read only; when no draft is open the header offers to open one from the
// latest version (RouteDraft.tsx). Every edit is a patch to the route's draft.
import "./authoring.css";

import { Link } from "react-router";

import { AuthoringPanel } from "./AuthoringPanel.tsx";
import { ancestorsOf, pathOf } from "./graph.ts";
import type { Authored } from "./target.ts";

/** A draft node's form in the inspector: its kind and place, then its sections. */
export function RouteNodePanel({ authored, nodeKey, close, onRemoved }: { authored: Authored; nodeKey: string; close: string; onRemoved: () => void }) {
  const node = authored.tree.byKey.get(nodeKey);
  if (node === undefined) {
    return (
      <aside className="detail-panel panel" data-testid="node-detail-missing">
        <p className="callout">The draft has no node {nodeKey}.</p>
        <Link to={close}>Close</Link>
      </aside>
    );
  }
  return (
    <aside className="detail-panel panel stack" aria-label={node.title} data-testid="route-node" data-node={nodeKey}>
      <div className="stack">
        <span className="muted mono">
          {node.kind}
          {ancestorsOf(authored.tree, nodeKey).length === 0 ? "" : ` in ${pathOf(authored.tree, nodeKey)}`}
        </span>
        <h2 data-testid="detail-title">{node.title}</h2>
      </div>
      <AuthoringPanel authored={authored} node={node} onRemoved={onRemoved} />
    </aside>
  );
}
