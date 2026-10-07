// A route's draft authored by hand on its canvas (A11, A12): the draft is always in edit mode,
// with the palette, the roles and participation kinds, drawing a requirement between two
// cards, and a node's structure in a panel beside the canvas at the route's address with the
// node's key. A published version is read only; when no draft is open the canvas offers to
// open one from the latest version. Every edit is a patch to the route's draft.
import "./authoring.css";

import { Link } from "react-router";

import type { Route } from "../data/host.ts";
import { openDraft } from "../routes/model.ts";
import { useScreenWrite } from "../screens/write.ts";
import { Badge, Button } from "../ui/kit.tsx";
import { AddNode } from "./AddNode.tsx";
import { AuthoringPanel } from "./AuthoringPanel.tsx";
import { DrawingBanner, type useEdgeDrawing } from "./connect.tsx";
import { ancestorsOf, pathOf } from "./graph.ts";
import { RolesAndKindsPanel } from "./RolesAndKindsPanel.tsx";
import type { Authored } from "./target.ts";

/** The draft's tools above its canvas: the palette, roles and kinds, and the edge being drawn. */
export function RouteAuthoringBar({ authored, container, onAdded, drawing }: { authored: Authored; container: string | undefined; onAdded: (key: string) => void; drawing: ReturnType<typeof useEdgeDrawing> }) {
  return (
    <section className="stack author-bar" aria-label="Edit the draft's structure" data-testid="authoring-bar">
      <AddNode authored={authored} container={container} onAdded={onAdded} />
      <RolesAndKindsPanel authored={authored} />
      <DrawingBanner drawing={drawing} />
    </section>
  );
}

/** A11: no draft is open, so the version shown is read only; one opens from the latest version. */
export function OpenDraftOffer({ route }: { route: Route }) {
  const write = useScreenWrite();
  return (
    <span className="row" data-testid="open-draft">
      <span className="muted">Published versions never change; edits go to a draft.</span>
      <Button disabled={write.disabled} onClick={() => void write.run({ target: { route: route.header.id }, baseRevision: route.revision, mutations: [openDraft()] })}>
        Open a draft to edit
      </Button>
    </span>
  );
}

/** A draft node's structure beside the canvas. */
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
      <div className="row">
        <h2 className="title" data-testid="detail-title">{node.title}</h2>
        <Badge>{node.kind}</Badge>
        <span className="shell-spacer" />
        <Link to={close} aria-label="Close the node">
          Close
        </Link>
      </div>
      <span className="muted mono">
        {ancestorsOf(authored.tree, nodeKey).length === 0 ? "" : "in "}
        {pathOf(authored.tree, nodeKey)}
      </span>
      <AuthoringPanel authored={authored} node={node} onRemoved={onRemoved} />
    </aside>
  );
}
