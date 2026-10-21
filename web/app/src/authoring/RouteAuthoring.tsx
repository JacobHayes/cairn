// A route's draft authored by hand on its canvas (A11, A12): the draft is always in edit mode,
// with the palette and the roles and participation kinds (StructureTools.tsx), drawing a requirement between two
// cards, and a node's form in the inspector at the draft's address with the node's key. A
// published version is read only: its nodes open in the inspector too, as what they say; when no
// draft is open the header offers to open one from the latest version (RouteDraft.tsx). Every
// edit is a patch to the route's draft.
import "./authoring.css";

import { Link } from "react-router";

import { useDeployment } from "../data/react.ts";
import { isStage, kindWord } from "../detail/model.ts";
import { Section, NodeLink } from "../detail/parts.tsx";
import { identityNamed } from "../detail/sections.tsx";
import { choiceLabel } from "../status/answer.ts";
import { Markdown } from "../ui/markdown.tsx";
import { AuthoringPanel } from "./AuthoringPanel.tsx";
import { conditionNames, conditionWords } from "./condition.ts";
import { ruleWords } from "./dates.ts";
import { ancestorsOf, childrenOf, pathOf, titleIn, type GraphNode, type Tree } from "./graph.ts";
import type { Authored } from "./target.ts";

/** A section of linked nodes, left out when there are none. */
function Linked({ title, testId, tree, keys, item }: { title: string; testId: string; tree: Tree; keys: readonly string[]; item: string | undefined }) {
  return keys.length === 0 ? null : (
    <Section title={title} summary={String(keys.length)} open={item !== undefined} testId={testId}>
      <ul className="detail-list">
        {keys.map((key) => (
          <li key={key} data-testid={item} data-node={key}>
            <NodeLink node={key} title={titleIn(tree, key)} />
          </li>
        ))}
      </ul>
    </Section>
  );
}

/** What a published version says of a node, read only: what it is, when it applies (its own condition and each container's it inherits), what it needs, what it holds, and its date rules. Sections with nothing to say are left out. */
function VersionNode({ tree, node }: { tree: Tree; node: GraphNode }) {
  const deployment = useDeployment();
  const names = conditionNames((decision) => tree.byKey.get(decision), (key) => (deployment === undefined ? key : identityNamed(deployment, key)));
  const conditions = [...ancestorsOf(tree, node.key), node].flatMap((holder) => (holder.relevant_when == null ? [] : [{ holder, words: conditionWords(holder.relevant_when, names) }]));
  const requires = node.requires ?? [];
  const holds = childrenOf(tree, node.key).map((child) => child.key);
  const rules = [
    node.due_by == null ? undefined : `Due ${ruleWords(node.due_by, (source) => titleIn(tree, source))}`,
    node.not_before == null ? undefined : `Starts ${ruleWords(node.not_before, (source) => titleIn(tree, source))}`,
  ].filter((words): words is string => words !== undefined);
  const choices = (node.choices ?? []).map(choiceLabel);
  return (
    <div className="stack" data-testid="version-node">
      {node.description == null || node.description === "" ? null : <Markdown text={node.description} data-testid="description" />}
      <div className="detail-folds">
        {node.kind === "decision" ? (
          <Section title="Question" summary={choices.length === 0 ? undefined : String(choices.length)} open testId="version-question">
            {node.prompt === undefined || node.prompt === "" ? null : <Markdown text={node.prompt} />}
            {choices.length === 0 ? null : (
              <ul className="detail-list">
                {choices.map((choice) => (
                  <li key={choice}>{choice}</li>
                ))}
              </ul>
            )}
          </Section>
        ) : null}
        {conditions.length === 0 ? null : (
          <Section title="When it applies" open testId="version-applies">
            <ul className="detail-list">
              {conditions.map(({ holder, words }) => (
                <li key={holder.key} data-node={holder.key}>
                  {`If ${words}`}
                  {holder.key === node.key ? "." : <> (from <NodeLink node={holder.key} title={holder.title} />).</>}
                </li>
              ))}
            </ul>
          </Section>
        )}
        <Linked title="Requirements" testId="version-requirements" tree={tree} keys={requires} item="requirement" />
        <Linked title="Holds" testId="version-holds" tree={tree} keys={holds} item={undefined} />
        {rules.length === 0 ? null : (
          <Section title="Dates" open testId="version-dates">
            {rules.map((words) => (
              <span key={words}>{words}</span>
            ))}
          </Section>
        )}
      </div>
      <p className="muted">A published version is read only. Open a draft to change it.</p>
    </div>
  );
}

/**
 * A route node in the inspector: its kind and place, then, on a draft, its form (`authored`), on
 * a published version what it says read only. A node the shown graph lacks is said without its key.
 */
export function RouteNodePanel({ tree, nodeKey, close, authored, onRemoved }: { tree: Tree; nodeKey: string; close: string; authored: Authored | undefined; onRemoved: () => void }) {
  const node = tree.byKey.get(nodeKey);
  if (node === undefined) {
    return (
      <aside className="detail-panel panel" data-testid="node-detail-missing">
        <p className="callout">{authored === undefined ? "This version has no such node." : "The draft has no such node."}</p>
        <Link to={close}>Close</Link>
      </aside>
    );
  }
  return (
    <aside className="detail-panel panel stack" aria-label={node.title} data-testid="route-node" data-node={nodeKey}>
      <div className="stack detail-head">
        <span className="muted mono">
          {kindWord(node.kind, isStage(node))}
          {ancestorsOf(tree, nodeKey).length === 0 ? "" : ` in ${pathOf(tree, nodeKey)}`}
        </span>
        <h2 data-testid="detail-title">{node.title}</h2>
      </div>
      {authored === undefined ? <VersionNode tree={tree} node={node} /> : <AuthoringPanel authored={authored} node={node} onRemoved={onRemoved} />}
    </aside>
  );
}
