// What every structure editor works on: one graph and where its patches go, a route's draft
// or a journey alike (PRD, How the pieces fit: one graph structure; A12: the canvas and node
// detail author both). A journey's carries its lineage, so its route-copied fields can be
// reset to the version it follows (B4).
import type { Schema } from "@cairn/client";

import type { Deployment, Route } from "../data/host.ts";
import type { Ready } from "../detail/model.ts";
import { treeOf, type Graph, type Tree } from "./graph.ts";

export type Lineage = Schema<"Lineage">;

/** A graph being authored and where its patches go. */
export interface Authored {
  /** The patch target: the journey, or the route whose draft this is. */
  target: { journey: string } | { route: string };
  graph: Graph;
  tree: Tree;
  /** The target's revision this graph is at: what an edit is drafted against (H5). */
  revision: number;
  /** The deployment revision a patch writing an entity reference names (E6). */
  deploymentRevision: number;
  deployment: Deployment;
  /** The deployment's today. */
  today: string;
  /** A journey's lineage, for its route-copied nodes (B4); none for a route or an empty journey. */
  lineage: Lineage | undefined;
  /** A route as read, which its local applies start from. */
  route: Route | undefined;
}

/** A journey as authoring sees it. */
export function journeyAuthored(ready: Ready): Authored {
  const graph = ready.journey.graph;
  return {
    target: { journey: ready.journey.header.id },
    graph,
    tree: treeOf(graph),
    revision: ready.journey.revision,
    deploymentRevision: ready.key.deployment_revision,
    deployment: ready.inputs.deployment,
    today: ready.key.today,
    lineage: ready.journey.header.lineage ?? undefined,
    route: undefined,
  };
}

/** A route's draft as authoring sees it; undefined when no draft is open. */
export function routeAuthored(route: Route, deployment: Deployment, today: string): Authored | undefined {
  const graph = route.draft?.graph;
  if (graph === undefined) {
    return undefined;
  }
  return {
    target: { route: route.header.id },
    graph,
    tree: treeOf(graph),
    revision: route.revision,
    deploymentRevision: deployment.revision,
    deployment,
    today,
    lineage: undefined,
    route,
  };
}

/** A name for the domain, for draft keys and test ids. */
export function domainOf(authored: Pick<Authored, "target">): string {
  return "journey" in authored.target ? `journey:${authored.target.journey}` : `route:${authored.target.route}`;
}

/** Whether the graph is a journey's. */
export function isJourney(authored: Pick<Authored, "target">): boolean {
  return "journey" in authored.target;
}

/** A role or kind as people read it: its title, or its id. */
export function named(item: { id: string; title?: string | null }): string {
  return item.title ?? item.id;
}
