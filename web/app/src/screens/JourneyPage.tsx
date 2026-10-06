// The shell's journey page: the journey's document derived in the derive worker, what it was
// derived from (revision, deployment revision, today: the query cache's key), and its nodes
// with their state and derived flags, each title editable through the shared write path.
// The node detail (5.1) and the canvas (5.2) build on and replace this list.
import { Link, useParams } from "react-router";

import type { JourneyView } from "../data/journeys.ts";
import { useJourney } from "../data/react.ts";
import { Badge, Panel } from "../ui/kit.tsx";
import { TitleEditor } from "./TitleEditor.tsx";

type Ready = Extract<JourneyView, { status: "ready" }>;

function Flags({ view, node }: { view: Ready; node: string }) {
  const derived = view.derived.nodes[node];
  if (derived === undefined) {
    return null;
  }
  return (
    <span className="row">
      {derived.actionable ? <Badge tone="good">actionable</Badge> : null}
      {derived.overdue === true ? <Badge tone="bad">overdue</Badge> : null}
      {(derived.stale ?? []).length > 0 ? <Badge tone="warn">stale</Badge> : null}
      {view.derived.frontier.includes(node) ? <Badge>frontier</Badge> : null}
    </span>
  );
}

function Nodes({ view }: { view: Ready }) {
  const { journey } = view;
  const states = journey.graph.state?.nodes ?? {};
  return (
    <table className="table">
      <thead>
        <tr>
          <th>Node</th>
          <th>Title</th>
          <th>State</th>
          <th>Derived</th>
        </tr>
      </thead>
      <tbody>
        {(journey.graph.nodes ?? []).map((node) => (
          <tr key={node.key} data-testid="node-row" data-node={node.key}>
            <td>
              <div className="mono">{node.key}</div>
              <div className="muted">{node.kind}</div>
            </td>
            <td>
              <TitleEditor journey={journey.header.id} node={node.key} title={node.title} revision={journey.revision} />
            </td>
            <td>{states[node.key]?.state ?? ""}</td>
            <td>
              <Flags view={view} node={node.key} />
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/**
 * The page for the journey the address names. Keyed by it, so going to another journey
 * starts every row afresh: its drafts and rejections are that journey's, even where two
 * journeys from one route share node keys.
 */
export function JourneyPage() {
  const { id = "" } = useParams();
  return <JourneyScreen key={id} id={id} />;
}

function JourneyScreen({ id }: { id: string }) {
  const view = useJourney(id);
  switch (view.status) {
    case "loading":
      return <p className="muted">Deriving the journey...</p>;
    case "missing":
      return <p className="callout" data-testid="journey-missing">This journey does not exist. <Link to="/">All journeys</Link></p>;
    case "failed":
      return <p className="callout callout-bad">The journey could not be read: {view.message}</p>;
    case "skew":
      return <p className="callout">This journey comes from a newer Cairn; reload to see it.</p>;
    case "ready":
      break;
  }
  const { header } = view.journey;
  const { key } = view;
  return (
    <Panel aria-label={header.name}>
      <div className="row">
        <h1 className="title" data-testid="journey-name">{header.name}</h1>
        <Badge>{header.status}</Badge>
      </div>
      <span className="muted" data-testid="derivation" data-revision={key.revision} data-deployment={key.deployment_revision}>
        Derived in this tab at revision {key.revision}, deployment revision {key.deployment_revision}, for {key.today};{" "}
        {view.derived.frontier.length} on the frontier.
      </span>
      <Nodes view={view} />
    </Panel>
  );
}
