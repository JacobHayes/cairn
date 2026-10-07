// A8: a date rule as its editor builds it: `due_by` (finish by) or `not_before` (start no
// sooner), measured before or after one or more sources by an offset in days, where a source
// is a milestone, a date decision's answer, or the journey's `created_at`; several sources
// are several constraints. F4: a stage's bounds are milestones outside it. The editors offer
// only what a rule may measure from, so a source of the wrong kind cannot be entered.
import type { DateRule } from "./fields.ts";
import { descendantsOf, nodesByPath, pathOf, type GraphNode, type Tree } from "./graph.ts";
import { OFFSET_DAYS_MAX, outOfRange } from "./limits.ts";

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

/** A8: what `node`'s rules may measure from: `created_at`, every milestone, and every date decision, other than itself. */
export function sourceOptions(tree: Tree, node: string | undefined): SourceOption[] {
  const nodes = nodesByPath(tree).filter(
    (each) => each.key !== node && (each.kind === "milestone" || (each.kind === "decision" && each.answer_type === "date")),
  );
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

/** Why the rule cannot be sent: no source, or an offset past the limit. */
export function ruleProblem(parts: RuleParts): string | undefined {
  if (parts.sources.length === 0) {
    return "A date rule measures from at least one source.";
  }
  return outOfRange(parts.offset, OFFSET_DAYS_MAX, "An offset in days");
}

/** A rule as people read it. */
export function ruleWords(rule: DateRule, label: (source: string) => string): string {
  const parts = partsOf(rule);
  const sources = parts.sources.map(label).join(", ");
  const days = parts.offset === 1 ? "1 day" : `${String(parts.offset)} days`;
  return parts.offset === 0 ? `${parts.direction === "before" ? "by" : "from"} ${sources}` : `${days} ${parts.direction} ${sources}`;
}

/**
 * F4: the milestones a stage may name as its bounds, those outside it first. One inside it is
 * offered too, marked: a bound sits outside the stage, and the engine says so at the field
 * (A15) rather than the editor hiding why it is not there.
 */
export function boundOptions(tree: Tree, group: string): { node: GraphNode; inside: boolean }[] {
  const inside = new Set(descendantsOf(tree, group).map((node) => node.key));
  const milestones = nodesByPath(tree, "milestone").map((node) => ({ node, inside: inside.has(node.key) }));
  return [...milestones.filter((each) => !each.inside), ...milestones.filter((each) => each.inside)];
}
