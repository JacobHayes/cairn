// The names the inspector and the screens beside it say things with (C8, E6): a role or a
// participation kind by its title, an identity by its name through any merge, an answer as people
// read it. The sections themselves are one file each (Connections, Affects, WhyRank, Facts).
import { answerWords, choiceNamed } from "../status/answer.ts";
import { dateWords } from "../status/when.ts";
import type { NodeDetail, Ready } from "./model.ts";
import { IDENTITY_GONE, KIND_GONE, ROLE_GONE, slotName } from "../ui/gone.ts";
import type { Schema } from "@cairn/client";
import { namedList } from "../ui/words.ts";
import { OWNER_KIND } from "./model.ts";

export type Slot = { key: string; id: string; title?: string | null | undefined };
type Deployment = Schema<"Deployment">;

/** A role by its title among `roles` (its id when it has none), or words for one that was removed. */
export function roleNamed(roles: readonly Slot[] | undefined, role: string): string {
  return slotName(roles?.find((each) => each.key === role), ROLE_GONE);
}

/** A participation kind by its title among `kinds`; the built-in owner kind has none in a graph. */
export function kindNamed(kinds: readonly Slot[] | undefined, kind: string): string {
  return kind === OWNER_KIND ? "Owner" : slotName(kinds?.find((each) => each.key === kind), KIND_GONE);
}

/** A journey's role by its title. */
export function roleTitle(view: Ready, role: string): string {
  return roleNamed(view.journey.graph.roles, role);
}

/** A journey's participation kind by its title. */
export function kindTitle(view: Ready, kind: string): string {
  return kindNamed(view.journey.graph.participation_kinds, kind);
}

/** E6: the identity a key names now in `deployment`, following the aliases a merge left behind. */
export function resolveAlias(deployment: Deployment, key: string): string {
  const aliases = deployment.aliases ?? {};
  let found = key;
  const seen = new Set<string>();
  while (aliases[found] !== undefined && !seen.has(found)) {
    seen.add(found);
    found = aliases[found] ?? found;
  }
  return found;
}

/** An identity's name from `deployment` (E6), through any merge, or words for one it does not hold. */
export function identityNamed(deployment: Deployment, key: string): string {
  const resolved = resolveAlias(deployment, key);
  return (deployment.identities ?? []).find((identity) => identity.key === resolved)?.name ?? IDENTITY_GONE;
}

/** E6: the identity a key names now in the journey's deployment. */
export function resolveIdentity(view: Ready, key: string): string {
  return resolveAlias(view.inputs.deployment, key);
}

/** An identity's name from the journey's deployment. */
export function identityName(view: Ready, key: string): string {
  return identityNamed(view.inputs.deployment, key);
}

/** E1, E2: whether one of the viewer's `identities`, through any merge, owns the node whose owners are `owners`. */
export function ownedBy(view: Ready, owners: readonly string[] | undefined, identities: readonly string[]): boolean {
  const mine = new Set(identities.map((key) => resolveIdentity(view, key)));
  return (owners ?? []).some((key) => mine.has(resolveIdentity(view, key)));
}

/** E2: the keys of a node's effective owners. */
export function ownersOf(view: Ready, key: string): string[] {
  return view.derived.nodes[key]?.participations?.[OWNER_KIND]?.identities ?? [];
}

/** What a surface says for the viewer among a node's owners. */
export const YOU = "You";
/** What a surface says for a node nobody owns. */
export const UNASSIGNED = "Unassigned";

/**
 * Who has a node, in words, on every surface: all its owners by name, the viewer (given as their
 * `viewer` identities) as "You", and "Unassigned" for none.
 */
export function ownerWords(view: Ready, owners: readonly string[] | undefined, viewer: readonly string[] = []): string {
  const keys = owners ?? [];
  if (keys.length === 0) {
    return UNASSIGNED;
  }
  const names = keys.filter((key) => !ownedBy(view, [key], viewer)).map((key) => identityName(view, key));
  return namedList(keys.length > names.length ? [YOU, ...names] : names, Infinity);
}

/** An answer as people read it: choices by their labels, identities by name. */
export function answerText(view: Ready, answer: NodeDetail["answer"] & object, decision?: NodeDetail["node"]): string {
  return answerWords(answer, { choice: (id) => choiceNamed(decision?.choices, id), identity: (key) => identityName(view, key), date: (date) => dateWords(date, view.derived.today) });
}
