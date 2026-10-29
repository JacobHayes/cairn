// Identities as the screens manage them (E6, B3, H3): made ad hoc with a name and enriched
// later with emails, edited, merged and deleted, each a deployment patch. A merge names every
// journey referring to any of the identities at the revision it was checked against, so the host
// can recheck the set when it commits; reads resolve the merged key through its alias afterwards.
import type { Schema } from "@cairn/client";

import { countCheck } from "../authoring/form-problems.ts";
import { askValues } from "../data/checks.ts";
import type { IdentityReferences } from "../data/host.ts";
import type { Mutation } from "../data/writes.ts";
import type { ChipRules } from "../ui/chips.ts";
import { identityLabel } from "./label.ts";

export type Identity = Schema<"Identity">;
export type Deployment = Schema<"Deployment">;

const BASE32 = "abcdefghijklmnopqrstuvwxyz234567";

/** A new identity's key: `who_` and sixteen random lowercase base32 characters, never derived from a name. */
export function newIdentityKey(random: (length: number) => Uint8Array = (length) => crypto.getRandomValues(new Uint8Array(length))): string {
  return `who_${Array.from(random(16), (byte) => BASE32[byte % 32]).join("")}`;
}

/** H3: the rules for an email chip: shown as the store holds it (trimmed and lower-cased); the engine says whether it is an email address, within the limit, and not already in the list. */
export function emailRules(session: Parameters<typeof askValues>[0]): ChipRules {
  return {
    normalize: (email) => email.trim().toLowerCase(),
    problem: async (email, held) => {
      const problems = await askValues(session, [
        { field: "email", kind: "email", what: `"${email}"`, value: email, taken: [...held] },
        countCheck("count", "email_count_per_identity_max", "emails in an identity", held.length + 1),
      ]);
      return problems["email"] ?? problems["count"];
    },
  };
}

/** E6, H3: an identity as it should be: its name trimmed (none when blank) and its emails (already normalized chips) as the list it holds. */
export function identityOf(key: string, name: string, emails: readonly string[]): Identity {
  const trimmed = name.trim();
  return { key, ...(trimmed === "" ? {} : { name: trimmed }), ...(emails.length === 0 ? {} : { emails: [...emails] }) };
}


/**
 * E6: the merge of `merged` into `survivor`, naming each journey that refers to any of them at the
 * revision seen. A journey two of them share is named once, at the lower revision read, so a
 * change between the reads makes the merge stale rather than unchecked.
 */
export function mergeMutation(survivor: string, merged: readonly string[], references: readonly IdentityReferences[]): Mutation {
  const journeys: Record<string, number> = {};
  for (const { journeys: referring } of references) {
    for (const summary of referring) {
      journeys[summary.id] = Math.min(journeys[summary.id] ?? summary.revision, summary.revision);
    }
  }
  return { op: "merge_identities", survivor, merged: [...merged], journeys };
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
