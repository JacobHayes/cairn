// Entities as the screens manage them (E6, B3, H3): made ad hoc with a name and enriched
// later with emails, edited, and merged, each a deployment patch. A merge names every journey
// referring to either entity at the revision it was checked against, so the host can recheck
// the set when it commits; reads resolve the merged key through its alias afterwards.
import type { Schema } from "@cairn/client";

import type { Mutation } from "../data/writes.ts";

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

/** H3: the emails typed into a field, one per comma, space, or line, trimmed, lower-cased, each once. */
export function emailsFrom(text: string): string[] {
  const emails = text
    .split(/[\s,;]+/)
    .map((email) => email.trim().toLowerCase())
    .filter((email) => email !== "");
  return [...new Set(emails)];
}

/** E6: an entity as it should be, its name trimmed and its emails read from `emails`. */
export function entityOf(key: string, name: string, emails: string): Entity {
  const listed = emailsFrom(emails);
  return { key, name: name.trim(), ...(listed.length === 0 ? {} : { emails: listed }) };
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
