// A journey's edit mode (B4: a journey may add, edit, move, and remove nodes, edges, roles,
// conditions, weights, and resources): a toggle on its canvas, kept in the address with what
// the canvas shows. In edit mode the canvas gains the palette that adds a node to the
// container drilled into, the journey's roles and participation kinds, the route nodes it
// removed (with restore), and drawing a requirement between two cards; node detail gains the
// node's structure (AuthoringPanel). A route's draft is always in edit mode
// (RouteAuthoring.tsx).
import "./authoring.css";

import { Link, useNavigate } from "react-router";

import { canvasPath, type CanvasView } from "../canvas/settings.ts";
import { AddNode } from "./AddNode.tsx";
import { DrawingBanner, type useEdgeDrawing } from "./connect.tsx";
import { RolesAndKindsPanel } from "./RolesAndKindsPanel.tsx";
import { Tombstones } from "./LocalEdits.tsx";
import type { Authored } from "./target.ts";

/** The link into or out of edit mode, keeping the rest of what the canvas shows. */
export function EditToggle({ journey, view, selected }: { journey: string; view: CanvasView; selected: string | undefined }) {
  return (
    <span className="row">
      <Link to={canvasPath(journey, { ...view, edit: !view.edit }, selected)} className="button" data-testid="edit-toggle" data-status={view.edit ? "on" : "off"}>
        {view.edit ? "Done editing structure" : "Edit structure"}
      </Link>
    </span>
  );
}

/** Edit mode's tools above the canvas: the palette, roles and kinds, tombstones, and the edge being drawn. */
export function JourneyAuthoringBar({
  authored,
  view,
  drawing,
}: {
  authored: Authored;
  view: CanvasView;
  drawing: ReturnType<typeof useEdgeDrawing>;
}) {
  const navigate = useNavigate();
  const journey = "journey" in authored.target ? authored.target.journey : "";
  return (
    <section className="stack author-bar" aria-label="Edit the journey's structure" data-testid="authoring-bar">
      <AddNode
        authored={authored}
        container={view.container}
        onAdded={(key) => {
          void navigate(canvasPath(journey, view, key));
        }}
      />
      <RolesAndKindsPanel authored={authored} />
      <Tombstones authored={authored} />
      <DrawingBanner drawing={drawing} />
    </section>
  );
}
