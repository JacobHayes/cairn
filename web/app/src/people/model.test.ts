// E6, H3: identity keys and emails as the screens send them, a merge naming every journey that
// refers to either identity at the revision seen, aliases resolving to the survivor, and the
// merge offered for a caller's duplicate identities.
import type { Schema } from "@cairn/client";
import { describe, expect, it } from "vitest";

import { linkable } from "./You.tsx";
import { aliasesOf, emailProblem, identityOf, mergeMutation, newIdentityKey, offeredMerge } from "./model.ts";

describe("identities as sent (E6, H3)", () => {
  it("mints keys the host accepts that differ from one another and carry no name", () => {
    const keys = Array.from({ length: 50 }, () => newIdentityKey());
    for (const key of keys) {
      expect(key).toMatch(/^who_[a-z2-7]{16}$/);
    }
    expect(new Set(keys).size).toBe(keys.length);
  });

  it("holds the emails it is given, none when none", () => {
    expect(identityOf("who_a", " Ann ", [])).toEqual({ key: "who_a", name: "Ann" });
    expect(identityOf("who_a", "Ann", ["ann@example.org"])).toEqual({ key: "who_a", name: "Ann", emails: ["ann@example.org"] });
  });

  it.each([
    ["ann@example.org", false],
    ["a@b", false],
    ["ann", true],
    ["@example.org", true],
    ["ann@", true],
    ["a@b@c.org", true],
    ["a b@c.org", true],
    [`${"x".repeat(250)}@e.org`, true],
  ])("judges %j a bad email: %s", (email, bad) => {
    expect(emailProblem(email) !== undefined).toBe(bad);
  });
});

describe("a merge (E6)", () => {
  it("names each referring journey at the revision it was read at", () => {
    const summary = (id: string, revision: number): Schema<"JourneySummary"> => ({ id, name: id, status: "archived", revision, created_at: "2026-10-01T00:00:00Z", upgrade_available: false });
    expect(mergeMutation("who_keep", "who_old", [summary("j_one", 4), summary("j_two", 9)])).toEqual({
      op: "merge_identities",
      survivor: "who_keep",
      merged: "who_old",
      journeys: { j_one: 4, j_two: 9 },
    });
  });

  it("lists the old keys resolving to an identity", () => {
    const deployment = { revision: 3, aliases: { who_old: "who_keep", who_older: "who_keep", who_else: "who_other" } };
    expect(aliasesOf(deployment, "who_keep")).toEqual(["who_old", "who_older"]);
    expect(aliasesOf(deployment, "who_none")).toEqual([]);
  });

  it("offers one merge at a time for a caller's duplicates, and none for one identity (H3)", () => {
    expect(offeredMerge(["who_b", "who_a", "who_c"])).toEqual({ survivor: "who_a", merged: "who_b" });
    expect(offeredMerge(["who_a"])).toBeUndefined();
    expect(offeredMerge(null)).toBeUndefined();
  });
});

describe("linking an identity (H3)", () => {
  it("offers a sign-in link for each OIDC provider only, so a host with none offers no link", () => {
    const capabilities = (...auth: { kind: string; name: string }[]) => ({ auth }) as Schema<"Capabilities">;
    expect(linkable(capabilities({ kind: "dev", name: "dev" }, { kind: "oidc", name: "stub" }))).toEqual(["stub"]);
    expect(linkable(capabilities({ kind: "dev", name: "dev" }))).toEqual([]);
  });
});
