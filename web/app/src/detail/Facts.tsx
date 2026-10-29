// C8, E2: who takes part in the node, and where it came from (held by the
// Details section): they explain rather than ask for anything.
import { memberOrigin, originLine } from "../segments/model.ts";
import { useSegmentNames } from "../segments/read.ts";
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

/** C8, B13: a segment member's origin: which segment and version, where the root went (text only). */
function SegmentOrigin({ view, node }: { view: Ready; node: string }) {
  const graph = view.journey.graph;
  const origin = memberOrigin(graph, node);
  const nameOf = useSegmentNames();
  if (origin === undefined) {
    return <span>From a segment.</span>;
  }
  const parent = origin.insertion.parent;
  const under = parent == null ? undefined : (graph.nodes ?? []).find((each) => each.key === parent)?.title;
  return <span data-testid="segment-origin">{originLine(origin, { segment: nameOf(origin.insertion.segment.route), parent: under })}</span>;
}

/** B13: where a segment's node came from; a node of the journey's own says nothing. */
export function Origin({ view, detail }: { view: Ready; detail: NodeDetail }) {
  return memberOrigin(view.journey.graph, detail.node.key) === undefined ? null : (
    <span className="stack" data-testid="provenance">
      <SegmentOrigin view={view} node={detail.node.key} />
    </span>
  );
}
