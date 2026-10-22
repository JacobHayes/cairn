// C16, A19: the journey index's selection in the inspector: "N selected" with Clear (on a phone,
// whom it names and whether they can go in a line under it, for the sheet's peek), each
// journey picked (its name a link, its status, and a × to drop it), then Delete over all of
// them, offered once every one is archived (DeleteJourneys.tsx).
import { Link } from "react-router";

import type { JourneySummary } from "../data/reads.ts";
import { Inspector } from "../shell/frame.tsx";
import { Button } from "../ui/kit.tsx";
import { journeyPath } from "./address.ts";
import { DeleteJourneys } from "./DeleteJourneys.tsx";
import { selectionGist } from "./lifecycle.ts";
import { JourneyStatusBadge } from "./StatusBadge.tsx";

export function SelectedJourneys({ members, toggle, clear }: { members: readonly JourneySummary[]; toggle: (journey: string) => void; clear: () => void }) {
  if (members.length === 0) {
    return null;
  }
  return (
    <Inspector focus={members.map((member) => member.id).join(" ")}>
      <div className="detail-panel panel stack" data-testid="selection-panel" data-count={members.length}>
        <div className="row">
          <strong>{members.length} selected</strong>
          <span className="spacer" />
          <Button ghost data-testid="selection-clear" onClick={clear}>Clear</Button>
        </div>
        <p className="muted small selection-gist">{selectionGist(members)}</p>
        <ul className="plain-list node-rows selected-members">
          {members.map((member) => (
            <li key={member.id} className="node-row" data-testid="selection-member" data-journey={member.id}>
              <div className="node-row-main">
                <span className="node-row-text">
                  <Link to={journeyPath({ journey: member.id })} className="node-title">{member.name}</Link>
                  <JourneyStatusBadge status={member.status} always />
                </span>
                <div className="row-side">
                  <Button ghost aria-label={`Remove ${member.name} from the selection`} title="Remove from the selection" onClick={() => { toggle(member.id); }}>×</Button>
                </div>
              </div>
            </li>
          ))}
        </ul>
        <DeleteJourneys journeys={members} onDeleted={clear} />
      </div>
    </Inspector>
  );
}
