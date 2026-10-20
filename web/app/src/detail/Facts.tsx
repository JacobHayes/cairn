// C8, E2, B4 (design 6.9): who takes part in the node, and where it came from (held by the
// Details section): they explain rather than ask for anything. The origin says whether the
// node is the route's, edited here, or the journey's own.
import { treeOf } from "../authoring/graph.ts";
import { editWords } from "../authoring/local-edits.ts";
import { memberOrigin, originLines } from "../segments/model.ts";
import { useLatestVersions, useSegmentNames } from "../segments/read.ts";
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

/** C8, B13: a segment member's origin: which segment and version, where the root went, and a newer version when there is one (text only). */
function SegmentOrigin({ view, node }: { view: Ready; node: string }) {
  const graph = view.journey.graph;
  const origin = memberOrigin(graph, node);
  const nameOf = useSegmentNames();
  const latest = useLatestVersions();
  if (origin === undefined) {
    return <span>From a segment.</span>;
  }
  const parent = origin.insertion.parent;
  const under = parent == null ? undefined : (graph.nodes ?? []).find((each) => each.key === parent)?.title;
  const lines = originLines(origin, { segment: nameOf(origin.insertion.segment.route), parent: under }, latest.get(origin.insertion.segment.route));
  return (
    <>
      {lines.map((line) => (
        <span key={line} data-testid="segment-origin">
          {line}
        </span>
      ))}
    </>
  );
}

/** B4, B7, B13: where the node came from, and what was changed here. */
export function Origin({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const { record, localEdits } = detail;
  const tree = treeOf(view.journey.graph);
  const edits = localEdits.map((edit) => editWords(edit, tree)).join(", ");
  return record.provenance === "from_segment" ? (
    <span className="stack" data-testid="provenance" data-status={record.provenance}>
      <SegmentOrigin view={view} node={detail.node.key} />
    </span>
  ) : (
    <span data-testid="provenance" data-status={record.provenance}>
      {record.provenance === "from_route" ? "From the route" : record.provenance === "orphaned" ? "Orphaned: its route version no longer has it" : "Local to this journey"}
      {edits === "" ? "." : `, edited here: ${edits}.`}
    </span>
  );
}
