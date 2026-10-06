// The shell's journey index (C16, first page): each journey with its status and revision,
// linking to its page, kept current (H6). The index screen proper (filters, paging, upgrade
// marks, the overview) is 5.5's and replaces this.
import { Link } from "react-router";

import { useJourneyIndex } from "../data/react.ts";
import { Badge, Panel } from "../ui/kit.tsx";

export function JourneyIndex() {
  const view = useJourneyIndex();
  if (view.status === "loading") {
    return <p className="muted">Loading the journeys...</p>;
  }
  if (view.status === "failed") {
    return <p className="callout callout-bad">The journeys could not be read: {view.message}</p>;
  }
  return (
    <Panel aria-label="Journeys">
      <h1 className="title">Journeys</h1>
      <table className="table">
        <thead>
          <tr>
            <th>Journey</th>
            <th>Status</th>
            <th>Revision</th>
          </tr>
        </thead>
        <tbody>
          {view.page.items.map((journey) => (
            <tr key={journey.id} data-testid="journey-row">
              <td>
                <Link to={`/journeys/${journey.id}`}>{journey.name}</Link>
                <div className="muted mono">{journey.id}</div>
              </td>
              <td>
                <Badge>{journey.status}</Badge>
              </td>
              <td className="mono">{journey.revision}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </Panel>
  );
}
