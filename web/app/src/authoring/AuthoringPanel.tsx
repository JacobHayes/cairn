// One node's structure, everything an author changes by hand (A12's by-hand path): its fields
// (NodeForm), where it sits, what it requires, who takes part by role, its resources, its
// breakdown, in a journey its departures from the route (B4), and its removal with the
// cascade (A18). The same panel serves a route's draft and a journey's edit mode.
import "./authoring.css";

import { Section } from "../detail/parts.tsx";
import { titleIn, type GraphNode } from "./graph.ts";
import { NodeForm } from "./NodeForm.tsx";
import { ResourceEditor } from "./ResourceEditor.tsx";
import { Breakdown, EdgesEditor, ParticipationWiring, PlaceEditor, RemoveNode } from "./StructureEditors.tsx";
import type { Authored } from "./target.ts";
import { isWork } from "../detail/model.ts";

export interface AuthoringPanelProps {
  authored: Authored;
  node: GraphNode;
  /** Called once the node is removed: its panel closes. */
  onRemoved: () => void;
}

export function AuthoringPanel({ authored, node, onRemoved }: AuthoringPanelProps) {
  const requires = node.requires ?? [];
  const resources = node.resources ?? [];
  const breaksDown = isWork(node);
  return (
    // Keyed by node, so every editor's state and kept draft are the open node's own.
    <div key={node.key} className="stack author-panel" data-testid="authoring" data-node={node.key}>
      <NodeForm key={node.key} authored={authored} node={node} />
      <div className="detail-folds">
        <Section title="Roles" summary={Object.keys(node.participations ?? {}).length === 0 ? undefined : String(Object.keys(node.participations ?? {}).length)} testId="author-participations">
          <ParticipationWiring authored={authored} node={node} />
        </Section>
        <Section title="Guidance" summary={resources.length === 0 ? undefined : String(resources.length)} testId="author-resources">
          <ResourceEditor authored={authored} node={node} />
        </Section>
        <Section title="Requirements" summary={requires.length === 0 ? undefined : String(requires.length)} testId="author-edges">
          <EdgesEditor authored={authored} node={node} />
        </Section>
        <Section title="Where it sits" summary={node.parent == null ? undefined : titleIn(authored.tree, node.parent)} testId="author-place">
          <PlaceEditor authored={authored} node={node} />
        </Section>
        {breaksDown ? (
          <Section title="Break it down" summary={node.placeholder === true ? "a placeholder" : undefined} open={node.placeholder === true} testId="author-breakdown">
            <Breakdown authored={authored} node={node} />
          </Section>
        ) : null}
      </div>
      <RemoveNode authored={authored} node={node} onRemoved={onRemoved} />
    </div>
  );
}
