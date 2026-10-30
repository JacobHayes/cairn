// C8, E2: who takes part in the node, and where it came from (held by the
// Details section): they explain rather than ask for anything.
import { Link } from "react-router";

import { useRouteLookup } from "../insert/read.ts";
import { routePath } from "../routes/address.ts";
import { originText } from "./explain.ts";
import { ParticipationEditor } from "./editors.tsx";
import { foldKey } from "./folds.ts";
import type { NodeDetail, Ready } from "./model.ts";
import { Section } from "./parts.tsx";
import { identityName, kindTitle } from "./sections.tsx";

/** Each participation kind's identities and where they come from (E2). */
export function People({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const kinds = Object.entries(detail.derived.participations ?? {});
  return (
    <Section title="People" summary={kinds.length === 0 ? undefined : String(kinds.length)} fold={foldKey(detail.node.kind, "people")} testId="participations">
      {kinds.length === 0 ? <span className="muted small">No one takes part yet.</span> : null}
      <ul className="detail-list">
        {kinds.map(([kind, participation]) => (
          <li key={kind} data-testid="participation" data-kind={kind}>
            <strong>{kindTitle(view, kind)}</strong>:{" "}
            {participation.identities.length === 0 ? "no one" : participation.identities.map((key) => identityName(view, key)).join(", ")}{" "}
            <span className="muted small">({originText(participation.origin, view)})</span>
          </li>
        ))}
      </ul>
      <ParticipationEditor view={view} detail={detail} />
    </Section>
  );
}

/** C8, B13: where a node copied in from a route came from, as the journey's own lineage says it: the route and version, linked. A node of the graph's own says nothing. */
export function Origin({ detail }: { detail: NodeDetail }) {
  const from = detail.node.from_route;
  const lookup = useRouteLookup();
  const found = from == null ? undefined : lookup(from.route);
  if (from == null || found === undefined) {
    return null;
  }
  return (
    <span className="stack" data-testid="provenance">
      {found === "gone" ? (
        <span data-testid="origin-route">From a route since deleted, version {from.version}</span>
      ) : (
        <span data-testid="origin-route">
          From route{" "}
          <Link to={routePath(from.route, { version: from.version })}>
            {found.name}, version {from.version}
          </Link>
        </span>
      )}
    </span>
  );
}
