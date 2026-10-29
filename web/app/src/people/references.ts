// E6: what refers to the identities selected on the Identities page, and what that lets the
// page offer. The references read (every journey, route draft and route version that names an
// identity, directly or through a merged key) is the host's, and it is the only authority for a
// delete: a read still going, one that failed, one that fell behind the deployment and one that
// lists anything each leave Delete off. A merge needs no empty read: it names what it finds.
import { IDENTITY_COUNT_PER_FILL_MAX } from "@cairn/client";

import type { IdentityReferences } from "../data/host.ts";
import type { ReadError, ReadView } from "../data/keyed-read.ts";
import { useLive } from "../data/react.ts";
import { referencesKey, referencesOf, type ReferencesByIdentity } from "../data/reads.ts";

/** What the references read of the selected identities has shown: still going, failed, or answered for every one. */
export type Reading = { kind: "reading" } | { kind: "failed"; error: ReadError } | { kind: "answered"; by: ReferencesByIdentity };

/**
 * What a read of the references has shown. Only a read current for the deployment as it is now
 * has answered: a held value being read again, or one whose newer read failed, is not an answer.
 */
export function readingOf(view: ReadView<ReferencesByIdentity>): Reading {
  switch (view.status) {
    case "current":
      return { kind: "answered", by: view.value };
    case "failed":
    case "behind":
      return { kind: "failed", error: view.error };
    case "gone":
      return { kind: "failed", error: { kind: "refused", message: "The identity is no longer there." } };
    default:
      return { kind: "reading" };
  }
}

/** The references of `identities`, read and kept current while they are selected; none selected reads nothing. */
export function useReferences(identities: readonly string[]): { reading: Reading; held: ReferencesByIdentity | undefined; refresh: () => void } {
  const { view, refresh } = useLive(identities.length === 0 ? undefined : referencesKey(identities), referencesOf(identities));
  const held = view.status === "current" || view.status === "refreshing" || view.status === "behind" ? view.value : undefined;
  return { reading: readingOf(view), held, refresh };
}

/** How many journeys and routes refer to an identity. */
export function referenceCount(references: IdentityReferences): number {
  return references.journeys.length + references.routes.length;
}

/** Whether Delete is on, and when it is not, why. `referenced` names the identities something refers to. */
export type Deletion = { kind: "allowed" } | { kind: "checking" } | { kind: "failed"; error: ReadError } | { kind: "referenced"; identities: string[] };

/** What the page offers for a selection. */
export interface Actions {
  /** Merging needs two or more, and at most the merged count one merge takes. */
  merge: { allowed: boolean; reason: "few" | "many" | undefined };
  delete: Deletion;
}

/**
 * E6: what the selection can be offered. Delete is allowed only when the references read has
 * answered for every member and none of them is referred to; a read going or failed never counts
 * as empty, and one that lists anything names who blocks it. Merge keeps one member and merges
 * the rest (at most `IDENTITY_COUNT_PER_FILL_MAX` of them).
 */
export function identityActions(members: readonly string[], reading: Reading): Actions {
  const merge = members.length < 2 ? { allowed: false, reason: "few" as const } : members.length - 1 > IDENTITY_COUNT_PER_FILL_MAX ? { allowed: false, reason: "many" as const } : { allowed: true, reason: undefined };
  if (members.length === 0) {
    return { merge, delete: { kind: "checking" } };
  }
  if (reading.kind !== "answered") {
    return { merge, delete: reading.kind === "failed" ? { kind: "failed", error: reading.error } : { kind: "checking" } };
  }
  const referenced = members.filter((member) => {
    const found = reading.by[member];
    // Answered for a selection, yet silent about a member: not an answer for that member.
    return found === undefined || referenceCount(found) > 0;
  });
  return { merge, delete: referenced.length === 0 ? { kind: "allowed" } : { kind: "referenced", identities: referenced } };
}

/** A route a reference names, in a few words: what it is and which of its draft and versions name the identity. */
export function routeFact(route: IdentityReferences["routes"][number]): string {
  const what = "Route";
  const where = [...(route.draft ? ["draft"] : []), ...(route.versions.length === 0 ? [] : [`${route.versions.length === 1 ? "version" : "versions"} ${route.versions.join(", ")}`])];
  return where.length === 0 ? what : `${what}, in its ${where.join(" and ")}`;
}
