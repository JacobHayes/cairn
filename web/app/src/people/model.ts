// Entities as the screens manage them (E6, B3, H3): made ad hoc with a name and enriched
// later with emails, edited, and merged, each a deployment patch. A merge names every journey
// referring to either entity at the revision it was checked against, so the host can recheck
// the set when it commits; reads resolve the merged key through its alias afterwards.
import type { Schema } from "@cairn/client";

import { pathTo } from "../address.ts";
import { EMAIL_BYTES_MAX, EMAIL_COUNT_PER_ENTITY_MAX } from "@cairn/client";
import { overBytes } from "../authoring/limits.ts";
import type { Mutation } from "../data/writes.ts";
import type { ChipRules } from "../ui/chips.ts";

export type Entity = Schema<"Entity">;
export type Deployment = Schema<"Deployment">;
type JourneySummary = Schema<"JourneySummary">;

/** A new entity's key: `e_`, the name as a slug, and a random suffix, so two never collide. */
export function newEntityKey(name: string, random: () => string = () => crypto.randomUUID().replaceAll("-", "").slice(0, 8)): string {
  const slug = name
    .toLowerCase()
    .normalize("NFKD")
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .slice(0, 40)
    .replace(/_+$/g, "");
  return `e_${slug === "" ? "entity" : slug}_${random()}`;
}

/** H3: the rules for an email chip: trimmed and lower-cased as the store holds it, shaped local@domain, at most 16 to an entity. */
export const EMAIL_RULES: ChipRules = {
  normalize: (email) => email.trim().toLowerCase(),
  problem: emailProblem,
  max: EMAIL_COUNT_PER_ENTITY_MAX,
  maxProblem: `An entity holds at most ${String(EMAIL_COUNT_PER_ENTITY_MAX)} emails.`,
};

/** H3: why `email` (normalized) is not an email address as the schema takes one (local@domain, no spaces), or undefined. */
export function emailProblem(email: string): string | undefined {
  const [local, domain, ...more] = email.split("@");
  const shaped = local !== undefined && local !== "" && domain !== undefined && domain !== "" && more.length === 0 && !/\s/.test(email);
  return shaped ? overBytes(email, EMAIL_BYTES_MAX, "The email") : `${email} is not an email address: write it as name@example.org.`;
}

/** E6: an entity as it should be, its name trimmed and its emails (already normalized chips) as the list it holds. */
export function entityOf(key: string, name: string, emails: readonly string[]): Entity {
  return { key, name: name.trim(), ...(emails.length === 0 ? {} : { emails: [...emails] }) };
}

/** E6: the merge of `merged` into `survivor`, naming each journey referring to either at the revision seen. */
export function mergeMutation(survivor: string, merged: string, referencing: readonly JourneySummary[]): Mutation {
  const journeys: Record<string, number> = {};
  for (const summary of referencing) {
    journeys[summary.id] = summary.revision;
  }
  return { op: "merge_entities", survivor, merged, journeys };
}

/** E6: the old keys that resolve to `key`: the entities merged into it. */
export function aliasesOf(deployment: Deployment, key: string): string[] {
  return Object.entries(deployment.aliases ?? {})
    .filter(([, target]) => target === key)
    .map(([alias]) => alias)
    .sort();
}

/** The entities by name, then key. */
export function byName(entities: readonly Entity[]): Entity[] {
  return [...entities].sort((left, right) => left.name.localeCompare(right.name) || left.key.localeCompare(right.key));
}

/**
 * H3: the merge the caller is offered when their verified emails name several entities: the
 * first (by key) survives and the next is merged into it; one merge at a time, until one is left.
 */
export function offeredMerge(offer: readonly string[] | null | undefined): { survivor: string; merged: string } | undefined {
  const [survivor, merged] = [...(offer ?? [])].sort();
  return survivor === undefined || merged === undefined ? undefined : { survivor, merged };
}

/** H3: the entities screen with the merge of `merged` into `survivor` open (`?merge=<survivor>,<merged>`). */
export function mergePath(survivor: string, merged: string): string {
  return `${pathTo("entities")}?${new URLSearchParams({ merge: `${survivor},${merged}` }).toString()}`;
}

/** The entities the entities screen's `search` asks to merge (`mergePath`), the survivor first. */
export function mergeAsked(search: string): string[] {
  return (new URLSearchParams(search).get("merge") ?? "").split(",").filter((key) => key !== "");
}
