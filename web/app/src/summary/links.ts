// Where the Summary's links go: every node, whether in a row, the lead or a cause, leads to the
// node's own address (it names the node, whatever the view), which opens its detail on the graph, so the Summary keeps no selection of
// its own. The one place the Summary builds a node address, so a change to the addresses is one edit.
import { DEFAULT_GRAPH_VIEW, graphPath } from "../canvas/settings.ts";
import { journeyPath } from "../journeys/address.ts";

/** The address of node `key` of `journey`, from the Summary. */
export function nodeAddress(journey: string, key: string): string {
  return journeyPath({ journey, node: key });
}

/** The journey's timeline, from the Summary's milestones. */
export function timelineAddress(journey: string): string {
  return journeyPath({ journey }, "timeline");
}

/** The graph in Edit mode, where an empty journey is planned. */
export function planAddress(journey: string): string {
  return graphPath(journey, { ...DEFAULT_GRAPH_VIEW, edit: true }, {}, "");
}
