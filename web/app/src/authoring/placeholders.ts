// A10, Keys and references: a message draft's placeholders. An author writes and reads
// them as a route file does (`{{roles.eval_owner.name}}`, `{{answers.testing/scope}}`, roles
// by id and decisions by path) and picks them from what the graph holds; the draft is stored
// with keys (`{{roles.r_eval_owner.name}}`, `{{answers.n_scope}}`), so renaming or moving a
// role or decision breaks nothing. The grammar is the engine's (`format::placeholders`), asked
// through the derive worker; a placeholder that names nothing the graph holds is caught before
// sending. What is here is the list an author picks from and the words for a refusal.
import { JOURNEY_FIELDS } from "@cairn/client";
import type { Schema } from "@cairn/client";
import type { DraftProblem } from "@cairn/wasm";

import { DERIVE, type ReadKey, type ReadSource } from "../data/keyed-read.ts";
import { useKeyedRead, useSession } from "../data/react.ts";
import type { Deriver } from "../data/host.ts";
import type { Role, Tree } from "./graph.ts";
import { nodesByPath, pathOf } from "./graph.ts";

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

/** Why a written draft cannot be stored, in words (the engine decides; this only phrases it). */
export function draftProblemWords(problem: DraftProblem): string {
  return "unreadable" in problem ? problem.unreadable : `Nothing in the graph answers ${problem.unknown.join(", ")}: pick a placeholder from the list.`;
}

interface Args {
  deriver: Deriver;
  graph: Schema<"Graph">;
  written: string;
}

const STORED: ReadSource<Args, { stored: string } | { problem: string }> = {
  policy: DERIVE,
  read: async ({ deriver, graph, written }) => {
    const answer = await deriver.draftStored(graph, written);
    return "stored" in answer ? answer : { problem: draftProblemWords(answer.problem) };
  },
};

/** What the engine makes of a draft as written: stored with keys, or why it cannot be; unchecked until it answers. */
export function useDraftStored(graph: Schema<"Graph">, written: string | undefined): { checked: boolean; result: { stored: string } | { problem: string } | undefined } {
  const { deriver } = useSession();
  const key: ReadKey<Args> | undefined = written === undefined ? undefined : { of: ["draft-stored"], freshness: [JSON.stringify(graph), written], args: { deriver, graph, written } };
  const { view } = useKeyedRead(STORED, key);
  if (written === undefined) {
    return { checked: true, result: undefined };
  }
  return view.status === "current" ? { checked: true, result: view.value } : { checked: false, result: undefined };
}
