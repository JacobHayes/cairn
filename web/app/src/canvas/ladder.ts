// C2, C4: the detail ladder (5.1). Two rules decide what is open and nothing else does: the
// step, and the viewer's own expands and collapses since picking it. At Stages, the current
// stages open to the Deliverables step and every other top-level stage is collapsed; at the other steps
// every container is open to that step. The level request carries the result (the kinds shown,
// the containers collapsed), so the engine does the roll-up (5.3). Pure, so a unit test reads
// the rules without a canvas.
import type { GraphNode, NodeKind } from "../detail/model.ts";
import { KINDS, STEPS, type CanvasSettings, type Step } from "./model.ts";

/** The kinds each step draws; Stages draws the Deliverables set inside the open stages, with its other stages collapsed. */
const KINDS_AT: Record<Step, NodeKind[]> = {
  stages: ["group", "decision", "deliverable", "milestone"],
  decisions: ["group", "decision", "milestone"],
  work: ["group", "decision", "deliverable", "milestone"],
  all: KINDS,
};

export const STEP_WORDS: Record<Step, string> = { stages: "Stages", decisions: "Decisions", work: "Deliverables", all: "All" };

/** C2: the kinds a top-level node must be at Stages for anything beneath it to be drawn. */
export const STAGES_TOP: NodeKind[] = ["group", "milestone"];

/** The kinds `step` draws. */
export function kindsAt(step: Step): NodeKind[] {
  return KINDS_AT[step];
}

/** A node as the ladder places it: its kind, its containers (nearest first) and the kind of the top-level node it sits under (its own when it is top-level). */
export interface Placed {
  kind: NodeKind;
  ancestors: readonly string[];
  top: NodeKind;
}

/** `kind` placed under `ancestors` (nearest first) among `nodes`. */
export function placedAt(nodes: readonly GraphNode[], kind: NodeKind, ancestors: readonly string[]): Placed {
  const root = ancestors.at(-1);
  return { kind, ancestors, top: nodes.find((node) => node.key === root)?.kind ?? kind };
}

/** C2: whether `step` draws a node: its kind is drawn, and at Stages its top-level node is a stage or a milestone. */
export function drawnAt(step: Step, node: Placed): boolean {
  return kindsAt(step).includes(node.kind) && (step !== "stages" || STAGES_TOP.includes(node.top));
}

/**
 * The nodes `step` leaves out (5.1): at Stages, each top-level node that is neither a stage nor a
 * milestone, with everything beneath it. Stages draws the Deliverables set only inside the stages.
 */
export function leftOutAt(step: Step, nodes: readonly GraphNode[]): Set<string> {
  const out = new Set<string>();
  if (step !== "stages") {
    return out;
  }
  const byKey = new Map(nodes.map((node) => [node.key, node]));
  const topOf = (node: GraphNode): GraphNode => {
    const seen = new Set<string>();
    let at = node;
    while (at.parent != null && !seen.has(at.key)) {
      seen.add(at.key);
      at = byKey.get(at.parent) ?? at;
    }
    return at;
  };
  for (const node of nodes) {
    const top = topOf(node);
    if (top.parent == null && !STAGES_TOP.includes(top.kind)) {
      out.add(node.key);
    }
  }
  return out;
}

/**
 * A route's ladder (8.11): a route has no current stage and no collapsed containers, so each
 * step is the kinds it draws and what the engine rolls up from the rest. Stages draws the
 * groups and milestones only.
 */
export function routeKindsAt(step: Step): NodeKind[] {
  return step === "stages" ? ["group", "milestone"] : KINDS_AT[step];
}

/**
 * The step a route's canvas is at: the one picked, else Stages, or Decisions when the route has no
 * groups, or Deliverables inside a container drilled into; a draft opens as a version does.
 */
export function routeStepOf(view: Pick<CanvasSettings, "step" | "container">, nodes: readonly GraphNode[]): Step {
  // Drilled into a stage, its work is what there is to see: Stages draws only groups and milestones.
  const picked = view.step ?? (view.container === undefined ? undefined : "work");
  if (picked !== undefined && (picked !== "stages" || hasStages(nodes))) {
    return picked;
  }
  return hasStages(nodes) ? "stages" : "decisions";
}

/** `step` when it draws `kind`, else the first step that does: where a node just added is on screen. */
export function routeStepDrawing(step: Step, kind: NodeKind): Step {
  return [step, ...STEPS].find((each) => routeKindsAt(each).includes(kind)) ?? "all";
}

/** The ladder's rungs for a journey: Stages is left out when it has no top-level groups. */
export function stepsFor(nodes: readonly GraphNode[]): Step[] {
  return hasStages(nodes) ? [...STEPS] : STEPS.filter((step) => step !== "stages");
}

/** Whether the journey has top-level groups (its stages). */
export function hasStages(nodes: readonly GraphNode[]): boolean {
  return nodes.some((node) => node.kind === "group" && node.parent == null);
}

/** The step a view is at: the one picked, else All. */
export function stepOf(view: Pick<CanvasSettings, "step">, nodes: readonly GraphNode[]): Step {
  const picked = view.step;
  if (picked !== undefined && (picked !== "stages" || hasStages(nodes))) {
    return picked;
  }
  return "all";
}

/** Whether `node` is a stage: a top-level group. */
export function isStage(node: GraphNode): boolean {
  return node.kind === "group" && node.parent == null;
}

/**
 * The stages that are current: those holding an acting-frontier or active node (5.1). `active`
 * and `frontier` are node keys; a stage holding one anywhere beneath it is current.
 */
export function currentStages(nodes: readonly GraphNode[], frontier: Iterable<string>, active: Iterable<string>): Set<string> {
  const byKey = new Map(nodes.map((node) => [node.key, node]));
  const current = new Set<string>();
  for (const key of [...frontier, ...active]) {
    let at = byKey.get(key);
    const seen = new Set<string>();
    while (at !== undefined && !seen.has(at.key)) {
      seen.add(at.key);
      if (isStage(at)) {
        current.add(at.key);
      }
      at = at.parent == null ? undefined : byKey.get(at.parent);
    }
  }
  return current;
}

/**
 * The containers the level collapses: at Stages every stage that is not current, unless the
 * viewer expanded it; at any step every container the viewer collapsed. A remembered collapse
 * of a node the graph no longer has (removed in another tab) is dropped: the projection rejects
 * unknown keys.
 */
export function collapsedAt(step: Step, nodes: readonly GraphNode[], current: ReadonlySet<string>, open: readonly string[], shut: readonly string[]): string[] {
  const known = new Set(nodes.map((node) => node.key));
  const collapsed = new Set(shut.filter((key) => known.has(key)));
  if (step === "stages") {
    for (const node of nodes) {
      if (isStage(node) && !current.has(node.key) && !open.includes(node.key)) {
        collapsed.add(node.key);
      }
    }
  }
  return [...collapsed].sort();
}

/**
 * C4, 5.1: whether the step opens container `key` before the viewer flips it: at Stages only the
 * current stages (`current`, as `currentStages` reads them) of the top-level ones, every other
 * container at every step. The list and the timeline fold by it as the graph collapses by
 * `collapsedAt`.
 */
export function openAt(step: Step, nodes: readonly GraphNode[], current: ReadonlySet<string>): (key: string) => boolean {
  const stages = new Set(nodes.filter(isStage).map((node) => node.key));
  return (key) => step !== "stages" || !stages.has(key) || current.has(key);
}

/** `view` after the viewer expands or collapses `key`: the click is remembered until a step is picked. */
export function withExpanded<T extends Pick<CanvasSettings, "open" | "shut">>(view: T, key: string, expanded: boolean): T {
  const without = (list: readonly string[]) => list.filter((each) => each !== key);
  return expanded ? { ...view, open: [...without(view.open), key], shut: without(view.shut) } : { ...view, shut: [...without(view.shut), key], open: without(view.open) };
}

/** `view` with `step` picked: it clears the viewer's expands and collapses (5.1). */
export function withStep<T extends Pick<CanvasSettings, "step" | "open" | "shut">>(view: T, step: Step): T {
  return { ...view, step, open: [], shut: [] };
}

/** The containers from `key`'s parent up, so a node can be shown in place. */
export function ancestorsOf(nodes: readonly GraphNode[], key: string): string[] {
  const byKey = new Map(nodes.map((node) => [node.key, node]));
  const ancestors: string[] = [];
  let at = byKey.get(key)?.parent ?? undefined;
  while (at !== undefined && !ancestors.includes(at)) {
    ancestors.push(at);
    at = byKey.get(at)?.parent ?? undefined;
  }
  return ancestors;
}
