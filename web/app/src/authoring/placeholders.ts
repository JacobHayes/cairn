// A10, Identity and references: a message draft's placeholders. An author writes and reads
// them as a route file does (`{{roles.eval_owner.name}}`, `{{answers.testing/scope}}`, roles
// by id and decisions by path) and picks them from what the graph holds; the draft is stored
// with keys (`{{roles.r_eval_owner.name}}`, `{{answers.n_scope}}`), so renaming or moving a
// role or decision breaks nothing. A placeholder that names nothing the graph holds is caught
// here, before sending.
import type { Role, Tree } from "./graph.ts";
import { nodesByPath, pathOf } from "./graph.ts";

/** A10: the journey fields a draft may name. */
export const JOURNEY_FIELDS = ["name", "description", "created_at", "url", "status"] as const;

const PLACEHOLDER = /\{\{\s*([^}]*?)\s*\}\}/g;

/** A placeholder an author can pick, as written and what it means. */
export interface PlaceholderOption {
  written: string;
  label: string;
}

/** Every placeholder the graph offers: journey fields, each role's names, each decision's answer. */
export function placeholderOptions(tree: Tree, roles: readonly Role[]): PlaceholderOption[] {
  return [
    ...JOURNEY_FIELDS.map((field) => ({ written: `{{journey.${field}}}`, label: `The journey's ${field.replace("_", " ")}` })),
    ...roles.map((role) => ({ written: `{{roles.${role.id}.name}}`, label: `Who fills ${role.title ?? role.id}` })),
    ...nodesByPath(tree, "decision").map((decision) => ({ written: `{{answers.${pathOf(tree, decision.key)}}}`, label: `The answer to "${decision.title}"` })),
  ];
}

/** A stored draft as an author reads it: roles by id, decisions by path. */
export function toWritten(stored: string, tree: Tree, roles: readonly Role[]): string {
  return stored.replace(PLACEHOLDER, (whole, inner: string) => {
    const role = /^roles\.(.+)\.name$/.exec(inner)?.[1];
    if (role !== undefined) {
      const found = roles.find((each) => each.key === role);
      return found === undefined ? whole : `{{roles.${found.id}.name}}`;
    }
    const answer = /^answers\.(.+)$/.exec(inner)?.[1];
    if (answer !== undefined && tree.byKey.has(answer)) {
      return `{{answers.${pathOf(tree, answer)}}}`;
    }
    return whole;
  });
}

/** What an author wrote, stored with keys, or why it cannot be. */
export function toStored(written: string, tree: Tree, roles: readonly Role[]): { stored: string } | { problem: string } {
  const opened = written.split("{{").length - 1;
  const closed = written.split("}}").length - 1;
  if (opened !== closed) {
    return { problem: "A '{{' has no closing '}}'." };
  }
  const decisions = new Map(nodesByPath(tree, "decision").map((decision) => [pathOf(tree, decision.key), decision.key]));
  const unknown: string[] = [];
  const stored = written.replace(PLACEHOLDER, (whole, inner: string) => {
    const field = /^journey\.(.+)$/.exec(inner)?.[1];
    if (field !== undefined && (JOURNEY_FIELDS as readonly string[]).includes(field)) {
      return `{{journey.${field}}}`;
    }
    const role = /^roles\.(.+)\.name$/.exec(inner)?.[1];
    const roleKey = roles.find((each) => each.id === role || each.key === role)?.key;
    if (roleKey !== undefined) {
      return `{{roles.${roleKey}.name}}`;
    }
    const answer = /^answers\.(.+)$/.exec(inner)?.[1];
    const decision = answer === undefined ? undefined : (decisions.get(answer) ?? (tree.byKey.get(answer)?.kind === "decision" ? answer : undefined));
    if (decision !== undefined) {
      return `{{answers.${decision}}}`;
    }
    unknown.push(whole);
    return whole;
  });
  return unknown.length === 0 ? { stored } : { problem: `Nothing in the graph answers ${unknown.join(", ")}: pick a placeholder from the list.` };
}
