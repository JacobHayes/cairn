// C8, Gating (design 6.10): a link between two nodes, selected on the graph: what it is in words,
// both ends as links with a way to go to each, and the sources of an implicit gate (which
// condition, stage, or ancestor produced it). An edge is addressed by its two nodes
// (`/edges/<from>~<to>`): it is a dependency of `to` on `from`.
import { Link, useLocation } from "react-router";

import { statusWord } from "../status/words.ts";
import { namer, viaText } from "./explain.ts";
import { nodeDetail, nodeOf, type Ready } from "./model.ts";
import { nodePath, NodeLink, screenPath } from "./parts.tsx";

/** `from~to` split into its two node keys, when it names two. */
export function edgeEnds(edge: string): { from: string; to: string } | undefined {
  const [from, to, ...rest] = edge.split("~");
  return from === undefined || to === undefined || from === "" || to === "" || rest.length > 0 ? undefined : { from, to };
}

/** How `to` depends on `from`: each dependency its derive lists, and the ancestor holding it when it is inherited. */
function sourcesOf(view: Ready, from: string, to: string) {
  const holders = [to, ...(view.derived.nodes[to]?.blocked_through ?? [])];
  return holders.flatMap((holder) => (view.derived.nodes[holder]?.blocked_by ?? []).filter((blocker) => blocker.node === from && blocker.via !== "containment").map((blocker) => blocker.via));
}

/**
 * What makes `to` depend on `from`, read from the graph rather than from what still holds, so a
 * link whose gate is satisfied is still one: an explicit requirement, a condition reading the
 * decision, a stage opening at the milestone, or either held by an ancestor.
 */
function structureOf(view: Ready, from: string, to: string): ReturnType<typeof sourcesOf> {
  const holders = [to, ...(nodeDetail(view, to)?.ancestors.map((ancestor) => ancestor.key) ?? [])];
  return holders.flatMap((holder) => {
    const own = nodeOf(view, holder);
    const relevance = view.derived.nodes[holder]?.relevance;
    return [
      ...(own?.requires?.includes(from) === true ? [holder === to ? ("explicit" as const) : { inherited: { ancestor: holder } }] : []),
      ...(own?.relevant_when != null && relevance?.decisions?.includes(from) === true ? [{ condition: { condition_on: holder } }] : []),
      ...(own?.opens_at === from ? [{ stage_opening: { group: holder } }] : []),
    ];
  });
}

/** The edge's kind as a label, for the band. */
function kindOf(via: ReturnType<typeof sourcesOf>[number] | undefined): string {
  if (via === undefined || via === "explicit") {
    return "Requires";
  }
  if (typeof via === "object" && "condition" in via) {
    return "Condition";
  }
  if (typeof via === "object" && "stage_opening" in via) {
    return "Stage opening";
  }
  return "Inherited requirement";
}

export function EdgeCard({ view, edge }: { view: Ready; edge: string }) {
  const ends = edgeEnds(edge);
  const { pathname, search } = useLocation();
  const from = ends === undefined ? undefined : nodeOf(view, ends.from);
  const to = ends === undefined ? undefined : nodeOf(view, ends.to);
  const missing = (
    <aside className="detail-panel panel stack" data-testid="edge-missing">
      <p className="callout">This journey has no such link.</p>
    </aside>
  );
  if (ends === undefined || from === undefined || to === undefined) {
    return missing;
  }
  const name = namer(view);
  const holding = sourcesOf(view, ends.from, ends.to);
  const sources = holding.length > 0 ? holding : structureOf(view, ends.from, ends.to);
  if (sources.length === 0) {
    return missing;
  }
  const screen = screenPath(pathname);
  return (
    <aside className="detail-panel panel stack" aria-label={`${from.title} to ${to.title}`} data-testid="edge-card" data-from={ends.from} data-to={ends.to}>
      <div className="stack detail-head">
        <span className="label">{kindOf(sources[0])}</span>
        <h2>
          {from.title} → {to.title}
        </h2>
      </div>
      <p className="sentence" data-testid="edge-sentence">
        <NodeLink view={view} node={ends.to} /> waits on <NodeLink view={view} node={ends.from} />
        {holding.length === 0 ? `, which is ${statusWord(view.derived.nodes[ends.from]?.display_state ?? "ready", from.kind).toLowerCase()}: it holds nothing back now.` : "."}
      </p>
      <div className="stack">
        <span className="muted small">Because of</span>
        <ul className="detail-list" data-testid="edge-sources">
          {sources.map((via) => (
            <li key={JSON.stringify(via)}>{viaText(via, name)}</li>
          ))}
        </ul>
      </div>
      <span className="row">
        <Link className="button" to={{ pathname: nodePath(screen, ends.from), search }}>
          Go to {from.title}
        </Link>
        <Link className="button" to={{ pathname: nodePath(screen, ends.to), search }}>
          Go to {to.title}
        </Link>
      </span>
    </aside>
  );
}
