// The names the inspector and the screens beside it say things with (C8, E6): a role or a
// participation kind by its title, an identity by its name through any merge, an answer as people
// read it. The sections themselves are one file each (Connections, Affects, WhyRank, Facts).
import { answerWords, choiceNamed } from "../status/answer.ts";
import { dateWords } from "../status/when.ts";
import type { NodeDetail, Ready } from "./model.ts";
import { IDENTITY_GONE, KIND_GONE, ROLE_GONE, slotName } from "../ui/gone.ts";
import { OWNER_KIND, type Schema } from "@cairn/client";
import { namedList } from "../ui/words.ts";
import { identityLabel } from "../people/label.ts";

export type Slot = { key: string; id: string; title?: string | null | undefined };
type Deployment = Schema<"Deployment">;

/** A role by its title among `roles` (its id when it has none), or words for one that was removed. */
export function roleNamed(roles: readonly Slot[] | undefined, role: string): string {
  return slotName(roles?.find((each) => each.key === role), ROLE_GONE);
}

/** A participation kind by its title among `kinds`; the built-in owner kind has none in a graph. */
export function kindNamed(kinds: readonly Slot[] | undefined, kind: string): string {
  return kind === OWNER_KIND.key ? "Owner" : slotName(kinds?.find((each) => each.key === kind), KIND_GONE);
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

/** An identity's name (else its first email, H3) from `deployment` (E6), through any merge, or words for one it does not hold. */
export function identityNamed(deployment: Deployment, key: string): string {
  const resolved = resolveAlias(deployment, key);
  const found = (deployment.identities ?? []).find((identity) => identity.key === resolved);
  return found === undefined ? IDENTITY_GONE : identityLabel(found);
}

/** E6: the identity a key names now in the journey's deployment. */
export function resolveIdentity(view: Ready, key: string): string {
  return resolveAlias(view.inputs.deployment, key);
}

/** An identity's name from the journey's deployment. */
export function identityName(view: Ready, key: string): string {
  return identityNamed(view.inputs.deployment, key);
}

/** E2: the keys of a node's effective owners. */
export function ownersOf(view: Ready, key: string): string[] {
  return view.derived.nodes[key]?.participations?.[OWNER_KIND.key]?.identities ?? [];
}

/** What a surface says for the viewer among a node's owners. */
export const YOU = "You";
/** What every surface says for a node or a journey nobody owns, short enough for a card's foot: the card, the inspector, the list, the rows and the Filter. */
export const NO_OWNER = "No owner";

/**
 * Identities as people say them, on every surface: `keys` through any merge, each identity once,
 * the viewer's own (`viewer`) first as `you` ("You" at the start of a sentence, "you" inside
 * one: the caller says which), the rest by name in the order given. Nothing for no keys.
 */
export function identityNameList(deployment: Deployment, keys: readonly string[], viewer: readonly string[], you: string): string[] {
  const mine = new Set(viewer.map((key) => resolveAlias(deployment, key)));
  const resolved = [...new Set(keys.map((key) => resolveAlias(deployment, key)))];
  const names = resolved.filter((key) => !mine.has(key)).map((key) => identityNamed(deployment, key));
  return resolved.some((key) => mine.has(key)) ? [you, ...names] : names;
}

/** `identityNameList` as one phrase of at most `max` names (the rest as "and N more"). */
export function identityNames(deployment: Deployment, keys: readonly string[], viewer: readonly string[], you: string, max = Infinity): string {
  return namedList(identityNameList(deployment, keys, viewer, you), max);
}

/**
 * Who has a node, in words, on every surface: all its owners by name, the viewer (given as their
 * `viewer` identities) as "You", and "No owner" for none.
 */
export function ownerWords(view: Ready, owners: readonly string[] | undefined, viewer: readonly string[] = []): string {
  const keys = owners ?? [];
  return keys.length === 0 ? NO_OWNER : identityNames(view.inputs.deployment, keys, viewer, YOU);
}

/** An answer as people read it: choices by their labels, identities by name. */
export function answerText(view: Ready, answer: NodeDetail["answer"] & object, decision?: NodeDetail["node"]): string {
  return answerWords(answer, { choice: (id) => choiceNamed(decision?.choices, id), identity: (key) => identityName(view, key), date: (date) => dateWords(date, view.derived.today) });
}
