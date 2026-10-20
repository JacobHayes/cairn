// E6: what the references read lets the Identities page offer. Delete is on only when the read
// has answered for every selected identity and none is named; a read going, failed, or silent
// about a member never counts as empty. A read that fell behind or is being read again has not
// answered either.
import { describe, expect, it } from "vitest";

import type { IdentityReferences } from "../data/host.ts";
import type { ReadView } from "../data/keyed-read.ts";
import type { ReferencesByIdentity } from "../data/reads.ts";
import { identityActions, readingOf, routeFact, type Reading } from "./references.ts";

const none: IdentityReferences = { journeys: [], routes: [] };
const inJourney: IdentityReferences = { journeys: [{ id: "j_a", name: "A", status: "active", revision: 1, created_at: "2026-10-01T00:00:00Z", upgrade_available: false }], routes: [] };
const answered = (by: ReferencesByIdentity): Reading => ({ kind: "answered", by });
const failure = { kind: "transient" as const, message: "down" };

describe("what Delete is offered for", () => {
  it("allows it when every member's read is empty", () => {
    expect(identityActions(["who_a", "who_b"], answered({ who_a: none, who_b: none })).delete).toEqual({ kind: "allowed" });
  });

  it("names who blocks it when anything names them, journeys or routes", () => {
    const route: IdentityReferences = { journeys: [], routes: [{ draft: true, versions: [], header: { id: "r_a", name: "R", kind: "process" } as never }] };
    expect(identityActions(["who_a", "who_b", "who_c"], answered({ who_a: none, who_b: inJourney, who_c: route })).delete).toEqual({ kind: "referenced", identities: ["who_b", "who_c"] });
  });

  it("is never allowed while the read is going or after it failed", () => {
    expect(identityActions(["who_a"], { kind: "reading" }).delete.kind).toBe("checking");
    expect(identityActions(["who_a"], { kind: "failed", error: failure }).delete.kind).toBe("failed");
  });

  it("does not count a member the answer is silent about as empty", () => {
    expect(identityActions(["who_a", "who_b"], answered({ who_a: none })).delete).toEqual({ kind: "referenced", identities: ["who_b"] });
  });

  it("offers a merge from two members to the most one merge takes", () => {
    const keys = (count: number) => Array.from({ length: count }, (_, at) => `who_${String(at)}`);
    expect(identityActions(keys(1), { kind: "reading" }).merge).toEqual({ allowed: false, reason: "few" });
    expect(identityActions(keys(2), { kind: "reading" }).merge.allowed).toBe(true);
    expect(identityActions(keys(101), { kind: "reading" }).merge.allowed).toBe(true);
    expect(identityActions(keys(102), { kind: "reading" }).merge).toEqual({ allowed: false, reason: "many" });
  });
});

describe("what a read of the references has shown", () => {
  const value: ReferencesByIdentity = { who_a: none };
  it.each<[string, ReadView<ReferencesByIdentity>, Reading["kind"]]>([
    ["idle", { status: "idle" }, "reading"],
    ["loading", { status: "loading" }, "reading"],
    ["refreshing a held value", { status: "refreshing", value }, "reading"],
    ["current", { status: "current", value }, "answered"],
    ["failed", { status: "failed", error: failure, retrying: true }, "failed"],
    ["behind with a held value", { status: "behind", value, error: failure, retrying: true }, "failed"],
    ["gone", { status: "gone" }, "failed"],
  ])("%s is %s", (_, view, kind) => {
    expect(readingOf(view).kind).toBe(kind);
  });
});

describe("a route a reference names", () => {
  const header = (kind: "process" | "segment") => ({ id: "r_a", name: "R", kind }) as never;
  it("tells a route from a segment, lists the versions that name the identity, and says draft only when the draft does", () => {
    const process = routeFact({ draft: false, versions: [2, 3], header: header("process") });
    expect(process).toContain("2, 3");
    expect(routeFact({ draft: true, versions: [2, 3], header: header("process") })).not.toBe(process);
    expect(routeFact({ draft: false, versions: [2, 3], header: header("segment") })).not.toBe(process);
  });
});
