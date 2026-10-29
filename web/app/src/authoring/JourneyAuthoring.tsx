// A journey's edit mode (B4: a journey may add, edit, move, and remove nodes, edges, roles,
// conditions, weights, and resources): the journey menu's Edit, kept in the address with what
// the canvas shows. In edit mode the page gains a band with the palette that adds a node to the
// container drilled into, the journey's roles and participation kinds, the route nodes it
// removed (with restore), and drawing a requirement between two cards; node detail gains the
// node's structure (AuthoringPanel). A route's draft is always in edit mode
// (RouteAuthoring.tsx).
import "./authoring.css";

import { Link, useLocation, useNavigate } from "react-router";

import { graphPath, type GraphView } from "../canvas/settings.ts";
import { DrawingBanner, type EdgeDrawing } from "./connect.tsx";
import { StructureTools } from "./StructureTools.tsx";
import { Tombstones } from "./LocalEdits.tsx";
import { HeaderEditor } from "../journeys/HeaderEditor.tsx";
import type { Ready } from "../detail/model.ts";
import type { Authored } from "./target.ts";

/** Edit mode's band under the header: what the edits are, the palette and the roles, Done; then tombstones and the edge being drawn. */
export function JourneyAuthoringBar({
  ready,
  authored,
  view,
  drawing,
}: {
  ready: Ready;
  authored: Authored;
  view: GraphView;
  drawing: EdgeDrawing;
}) {
  const navigate = useNavigate();
  const { search } = useLocation();
  const journey = "journey" in authored.target ? authored.target.journey : "";
  return (
    <section className="stack author-bar" aria-label="Edit the journey's structure" data-testid="authoring-bar">
      <div className="row">
        <span>
          <strong>Editing</strong> this journey
        </span>
        <span className="spacer" />
        <StructureTools
          authored={authored}
          container={undefined}
          onAdded={(node) => {
            // The node just added is selected, so the view draws it whatever its step, filter and search would hide.
            void navigate(graphPath(journey, view, { node: node.key }, search));
          }}
        />
        <Link to={graphPath(journey, { ...view, edit: false }, {}, search)} className="button" data-testid="edit-done">
          Done
        </Link>
      </div>
      <HeaderEditor ready={ready} />
      <Tombstones authored={authored} />
      <DrawingBanner drawing={drawing} />
    </section>
  );
}
