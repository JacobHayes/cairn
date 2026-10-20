// Identities as the screens manage them (E6, B3, H3): made ad hoc with a name and enriched
// later with emails, edited, and merged, each a deployment patch. A merge names every journey
// referring to either identity at the revision it was checked against, so the host can recheck
// the set when it commits; reads resolve the merged key through its alias afterwards.
import type { Schema } from "@cairn/client";

import { EMAIL_BYTES_MAX, EMAIL_COUNT_PER_IDENTITY_MAX } from "@cairn/client";
import { overBytes } from "../authoring/limits.ts";
import type { Mutation } from "../data/writes.ts";
import type { ChipRules } from "../ui/chips.ts";
import { identityLabel } from "./label.ts";

export type Identity = Schema<"Identity">;
export type Deployment = Schema<"Deployment">;
type JourneySummary = Schema<"JourneySummary">;

const BASE32 = "abcdefghijklmnopqrstuvwxyz234567";

/** A new identity's key: `who_` and sixteen random lowercase base32 characters, never derived from a name. */
export function newIdentityKey(random: (length: number) => Uint8Array = (length) => crypto.getRandomValues(new Uint8Array(length))): string {
  return `who_${Array.from(random(16), (byte) => BASE32[byte % 32]).join("")}`;
}

/** H3: the rules for an email chip: trimmed and lower-cased as the store holds it, shaped local@domain, at most 16 to an identity. */
export const EMAIL_RULES: ChipRules = {
  normalize: (email) => email.trim().toLowerCase(),
  problem: emailProblem,
  max: EMAIL_COUNT_PER_IDENTITY_MAX,
  maxProblem: `An identity holds at most ${String(EMAIL_COUNT_PER_IDENTITY_MAX)} emails.`,
};

/** H3: why `email` (normalized) is not an email address as the schema takes one (local@domain, no spaces), or undefined. */
export function emailProblem(email: string): string | undefined {
  const [local, domain, ...more] = email.split("@");
  const shaped = local !== undefined && local !== "" && domain !== undefined && domain !== "" && more.length === 0 && !/\s/.test(email);
  return shaped ? overBytes(email, EMAIL_BYTES_MAX, "The email") : `${email} is not an email address: write it as name@example.org.`;
}

/** E6, H3: an identity as it should be: its name trimmed (none when blank) and its emails (already normalized chips) as the list it holds. */
export function identityOf(key: string, name: string, emails: readonly string[]): Identity {
  const trimmed = name.trim();
  return { key, ...(trimmed === "" ? {} : { name: trimmed }), ...(emails.length === 0 ? {} : { emails: [...emails] }) };
}


/** E6: the merge of `merged` into `survivor`, naming each journey referring to either at the revision seen. */
export function mergeMutation(survivor: string, merged: string, referencing: readonly JourneySummary[]): Mutation {
  const journeys: Record<string, number> = {};
  for (const summary of referencing) {
    journeys[summary.id] = summary.revision;
  }
  return { op: "merge_identities", survivor, merged, journeys };
}

/** E6: the old keys that resolve to `key`: the identities merged into it. */
export function aliasesOf(deployment: Deployment, key: string): string[] {
  return Object.entries(deployment.aliases ?? {})
    .filter(([, target]) => target === key)
    .map(([alias]) => alias)
    .sort();
}

/** The identities by how they are shown (a name, else the first email), then key. */
export function byName(identities: readonly Identity[]): Identity[] {
  return [...identities].sort((left, right) => identityLabel(left).localeCompare(identityLabel(right)) || left.key.localeCompare(right.key));
}
