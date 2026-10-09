// The names the inspector and the screens beside it say things with (C8, E6): a role or a
// participation kind by its title, an entity by its name through any merge, an answer as people
// read it. The sections themselves are one file each (Connections, Affects, WhyRank, Facts).
import { answerWords } from "./explain.ts";
import type { NodeDetail, Ready } from "./model.ts";

/** A role by its title, or its key when it has none. */
export function roleTitle(view: Ready, role: string): string {
  return (view.journey.graph.roles ?? []).find((each) => each.key === role)?.title ?? role;
}

/** A participation kind by its title (the built-in owner kind has none in the graph), or its key when it has none. */
export function kindTitle(view: Ready, kind: string): string {
  return kind === "k_owner" ? "Owner" : ((view.journey.graph.participation_kinds ?? []).find((each) => each.key === kind)?.title ?? kind);
}

/** E6: the entity a key names now, following the aliases a merge left behind. */
export function resolveEntity(view: Ready, key: string): string {
  const aliases = view.inputs.deployment.aliases ?? {};
  let found = key;
  const seen = new Set<string>();
  while (aliases[found] !== undefined && !seen.has(found)) {
    seen.add(found);
    found = aliases[found] ?? found;
  }
  return found;
}

/** An entity's name from the deployment (E6), through any merge, or its key. */
export function entityName(view: Ready, key: string): string {
  const resolved = resolveEntity(view, key);
  return (view.inputs.deployment.entities ?? []).find((entity) => entity.key === resolved)?.name ?? key;
}

/** An answer as people read it: choices by their labels, entities by name. */
export function answerText(view: Ready, answer: NodeDetail["answer"] & object, decision?: NodeDetail["node"]): string {
  const label = (id: string) => {
    const choice = (decision?.choices ?? []).find((each) => (typeof each === "string" ? each : each.id) === id);
    return choice === undefined || typeof choice === "string" ? id : choice.title;
  };
  if ("single_choice" in answer) {
    return label(answer.single_choice);
  }
  if ("multi_choice" in answer) {
    return answer.multi_choice.length === 0 ? "none" : answer.multi_choice.map(label).join(", ");
  }
  if ("entity" in answer) {
    return entityName(view, answer.entity);
  }
  if ("entity_list" in answer) {
    return answer.entity_list.length === 0 ? "none" : answer.entity_list.map((key) => entityName(view, key)).join(", ");
  }
  return answerWords(answer);
}
