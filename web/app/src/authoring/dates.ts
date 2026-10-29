// A8: a date rule as its editor builds it: `due_by` (finish by) or `not_before` (start no
// sooner), measured before or after one or more sources by an offset in days, where a source
// is a milestone, a date decision's answer, or the journey's `created_at`; several sources
// are several constraints. F4: a stage's bounds are milestones outside it. The editors offer
// only what the engine's table says a rule may measure from, so a source of the wrong kind
// cannot be entered.
import type { DateRule } from "./fields.ts";
import { descendantsOf, nodesFor, pathOf, titleIn, type GraphNode, type Tree } from "./graph.ts";
import { plural } from "../ui/words.ts";

/** The journey's creation, a source every graph has (A8). */
export const CREATED_AT = "journey.created_at";

export type Direction = "before" | "after";

/** A rule's parts as its editor holds them. */
export interface RuleParts {
  direction: Direction;
  sources: string[];
  offset: number;
}

/** A source an editor offers, with what it is called. */
export interface SourceOption {
  source: string;
  label: string;
}

/** A8: what `node`'s `which` rule may measure from: `created_at`, and the nodes the engine's table allows (milestones and date decisions), other than itself. */
export function sourceOptions(tree: Tree, node: string | undefined, which: "due_by" | "not_before"): SourceOption[] {
  const nodes = nodesFor(tree, which).filter((each) => each.key !== node);
  return [
    { source: CREATED_AT, label: "When the journey started" },
    ...nodes.map((each) => ({ source: each.key, label: `${each.title} (${pathOf(tree, each.key)})` })),
  ];
}

/** A rule's parts. */
export function partsOf(rule: DateRule): RuleParts {
  const direction: Direction = rule.before === undefined ? "after" : "before";
  const raw = rule.before ?? rule.after ?? [];
  return { direction, sources: Array.isArray(raw) ? [...raw] : [raw], offset: rule.offset ?? 0 };
}

/** The rule its parts make: one source written bare, several as a list. */
export function ruleOf(parts: RuleParts): DateRule {
  const sources = parts.sources.length === 1 ? parts.sources[0] : parts.sources;
  const rule: DateRule = parts.direction === "before" ? { before: sources ?? [] } : { after: sources ?? [] };
  return parts.offset === 0 ? rule : { ...rule, offset: parts.offset };
}

/** A rule as people read it; `label` names a node by its title, and the journey's start is named here. */
export function ruleWords(rule: DateRule, label: (source: string) => string): string {
  const parts = partsOf(rule);
  const sources = parts.sources.map((source) => (source === CREATED_AT ? "the journey's start" : label(source))).join(" and ");
  const days = plural(parts.offset, "day");
  return parts.offset === 0 ? `${parts.direction === "before" ? "by" : "from"} ${sources}` : `${days} ${parts.direction} ${sources}`;
}

/**
 * What a route card's foot says of the node's date rule, in words: "Due 14 days before Decision
 * meeting", or "Starts 3 days after Kickoff" when it has only a not-before. None without a rule.
 */
export function ruleFoot(node: GraphNode, tree: Tree): string | undefined {
  const [rule, lead] = node.due_by != null ? ([node.due_by, "Due"] as const) : node.not_before != null ? ([node.not_before, "Starts"] as const) : [undefined, ""];
  if (rule === undefined) {
    return undefined;
  }
  return `${lead} ${ruleWords(rule, (source) => titleIn(tree, source))}`;
}

/**
 * F4: the nodes a stage may name as its `bound` (the engine's table: milestones), those outside it
 * first. One inside it is offered too, marked: a bound sits outside the stage, and the engine says
 * so at the field (A15) rather than the editor hiding why it is not there.
 */
export function boundOptions(tree: Tree, group: string, bound: "opens_at" | "closes_at"): { node: GraphNode; inside: boolean }[] {
  const inside = new Set(descendantsOf(tree, group).map((node) => node.key));
  const milestones = nodesFor(tree, bound).map((node) => ({ node, inside: inside.has(node.key) }));
  return [...milestones.filter((each) => !each.inside), ...milestones.filter((each) => each.inside)];
}
