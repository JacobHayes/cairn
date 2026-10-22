// A18: `CascadeDialog` shows a removal whole before it is sent: the node and everything
// beneath it, every edge into or out of them, what hangs on them (resources, participations,
// a journey's notes and links), and each reference elsewhere that would dangle with what the
// same patch does about it. The patch is previewed with the engine first, so the author sees
// whether it is accepted and what it would newly cause in the journey (D7) before confirming.
// Removing a role or a kind shows its cascade the same way. Exported for proposal review
// (5.7), which shows cascaded items the same way.

import { Failed } from "../ui/Failed.tsx";
import { Button } from "../ui/kit.tsx";
import type { Dangling, Removal, RemovalPlan } from "./removal.ts";
import type { Mutation } from "./graph.ts";
import { titleIn } from "./graph.ts";
import type { Authored } from "./target.ts";
import { reasonOf } from "../data/writes.ts";
import { RejectionView, ViolationLines } from "../screens/RejectionView.tsx";
import { WriteStatus } from "../screens/WriteStatus.tsx";
import { useAuthorWrite, useDialogPreview, type Preview } from "./write.ts";
import { plural } from "../ui/words.ts";

export interface CascadeDialogProps {
  authored: Authored;
  /** Which removal this is, so its write is its own beside another dialog's. */
  scope: string;
  /** What is removed, in words ("Remove Testing"). */
  title: string;
  /** A node removal's plan; none for a role or a kind. */
  plan?: RemovalPlan | undefined;
  dangling: readonly Dangling[];
  /** Everything the patch sends, rewrites first. */
  mutations: Mutation[];
  onDone: () => void;
  onCancel: () => void;
}

function count(items: readonly unknown[] | undefined, one: string, many: string): string | undefined {
  const length = items?.length ?? 0;
  return length === 0 ? undefined : plural(length, one, many);
}

/** The subtree, its edges, and what hangs on it. */
function Removed({ authored, plan, removal }: { authored: Authored; plan: RemovalPlan; removal: Removal }) {
  const attached = [count(removal.resources, "resource", "resources"), count(removal.participations, "participation", "participations"), count(removal.annotations, "note or link", "notes and links")].filter(
    (part) => part !== undefined,
  );
  return (
    <div className="stack" data-testid="cascade-removed">
      <strong>Removes</strong>
      <ul className="detail-list">
        {plan.nodes.map((node) => (
          <li key={node.key} data-testid="cascade-node" data-node={node.key}>
            {node.title} <span className="muted small">({node.kind})</span>
          </li>
        ))}
      </ul>
      {(removal.edges ?? []).length === 0 ? null : (
        <ul className="detail-list" aria-label="Edges removed">
          {(removal.edges ?? []).map((edge) => (
            <li key={`${edge.node}>${edge.requires}`} data-testid="cascade-edge">
              {titleIn(authored.tree, edge.node)} requires {titleIn(authored.tree, edge.requires)}
            </li>
          ))}
        </ul>
      )}
      {attached.length === 0 ? null : <span className="muted small">With them: {attached.join(", ")}.</span>}
    </div>
  );
}

/** What the preview says: accepted, with what it would newly cause, or every violation. */
function PreviewLine({ preview }: { preview: Preview | undefined }) {
  if (preview === undefined) {
    return <span className="muted small">Checking with the engine...</span>;
  }
  if (preview.outcome === "unavailable") {
    return <span className="muted small"><Failed what="The preview" verb="made" error={preview.message} /></span>;
  }
  if (preview.outcome === "rejected") {
    const { rejection } = preview;
    return (
      <div className="stack callout callout-bad" data-testid="cascade-preview" data-status="rejected">
        {rejection.rejection === "invalid" ? <ViolationLines violations={rejection.violations} /> : <span className="author-problem">{reasonOf(rejection)}</span>}
      </div>
    );
  }
  const caused = preview.consequences;
  const newly = [count(caused?.stale, "node becomes stale", "nodes become stale"), count(caused?.shortfalls, "new shortfall", "new shortfalls"), count(caused?.overdue, "node becomes overdue", "nodes become overdue")].filter(
    (part) => part !== undefined,
  );
  return (
    <span className="muted small" data-testid="cascade-preview" data-status="accepted">
      The engine accepts this{newly.length === 0 ? "." : `; ${newly.join(", ")}.`}
    </span>
  );
}

export function CascadeDialog({ authored, scope, title, plan, dangling, mutations, onDone, onCancel }: CascadeDialogProps) {
  const write = useAuthorWrite(authored, `cascade:${scope}`);
  const preview = useDialogPreview(authored, mutations);
  return (
    <div className="callout stack" role="dialog" aria-label={title} data-testid="cascade-dialog">
      <strong>{title}</strong>
      {plan?.removal === undefined ? null : <Removed authored={authored} plan={plan} removal={plan.removal} />}
      {dangling.length === 0 ? null : (
        <div className="stack">
          <strong>Also changes, in the same patch</strong>
          <ul className="detail-list">
            {dangling.map((each, at) => (
              <li key={at} data-testid="cascade-rewrite" data-what={each.what} data-node={each.node}>
                {each.node === undefined ? "The graph" : titleIn(authored.tree, each.node)}: {each.fix}
              </li>
            ))}
          </ul>
        </div>
      )}
      <PreviewLine preview={preview} />
      <span className="row">
        <Button
          primary
          disabled={write.disabled || preview?.outcome === "rejected"}
          onClick={() => void write.run(mutations).then((landed) => { if (landed) { onDone(); } })}
        >
          Confirm
        </Button>
        <Button onClick={onCancel}>Cancel</Button>
      </span>
      <WriteStatus view={write.status} />
      {write.failed === undefined ? null : <RejectionView rejection={write.failed.rejection} onDismiss={write.dismiss} />}
    </div>
  );
}
