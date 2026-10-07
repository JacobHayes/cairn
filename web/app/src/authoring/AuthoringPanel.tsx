// One node's structure, everything an author changes by hand (A12's by-hand path): its fields
// (NodeForm), where it sits, what it requires, who takes part by role, its resources, its
// breakdown, in a journey its departures from the route (B4), and its removal with the
// cascade (A18). The same panel serves a route's draft and a journey's edit mode.
import "./authoring.css";

import { Section } from "../detail/parts.tsx";
import type { GraphNode } from "./graph.ts";
import { LocalEdits } from "./LocalEdits.tsx";
import { NodeForm } from "./NodeForm.tsx";
import { ResourceEditor } from "./ResourceEditor.tsx";
import { Breakdown, EdgesEditor, ParticipationWiring, PlaceEditor, RemoveNode } from "./StructureEditors.tsx";
import { isJourney, type Authored } from "./target.ts";

export interface AuthoringPanelProps {
  authored: Authored;
  node: GraphNode;
  /** Called once the node is removed: its panel closes. */
  onRemoved: () => void;
}

export function AuthoringPanel({ authored, node, onRemoved }: AuthoringPanelProps) {
  const requires = node.requires ?? [];
  const resources = node.resources ?? [];
  const breaksDown = node.kind === "deliverable" || node.kind === "action";
  return (
    // Keyed by node, so every editor's state and kept draft are the open node's own.
    <div key={node.key} className="stack author-panel" data-testid="authoring" data-node={node.key}>
      <RemoveNode authored={authored} node={node} onRemoved={onRemoved} />
      <Section title="Fields" summary={`a ${node.kind}'s own`} open testId="author-fields">
        <NodeForm key={node.key} authored={authored} node={node} />
      </Section>
      <Section title="Where it sits" summary={node.parent == null ? "at the top level" : `inside ${authored.tree.byKey.get(node.parent)?.title ?? node.parent}`} testId="author-place">
        <PlaceEditor authored={authored} node={node} />
      </Section>
      <Section title="Requirements" summary={requires.length === 0 ? "none explicit" : `${String(requires.length)} explicit`} testId="author-edges">
        <EdgesEditor authored={authored} node={node} />
      </Section>
      <Section title="Who takes part, by role" summary={`${String(Object.keys(node.participations ?? {}).length)} declared`} testId="author-participations">
        <ParticipationWiring authored={authored} node={node} />
      </Section>
      <Section title="Resources" summary={resources.length === 0 ? "none" : String(resources.length)} testId="author-resources">
        <ResourceEditor authored={authored} node={node} />
      </Section>
      {breaksDown ? (
        <Section title="Break it down" summary={node.placeholder === true ? "a placeholder" : "by hand"} open={node.placeholder === true} testId="author-breakdown">
          <Breakdown authored={authored} node={node} />
        </Section>
      ) : null}
      {isJourney(authored) ? (
        <Section title="Away from the route" summary={`${String((authored.graph.state?.local_edits?.[node.key] ?? []).length)} edited here`} testId="author-local-edits">
          <LocalEdits authored={authored} node={node} />
        </Section>
      ) : null}
    </div>
  );
}
