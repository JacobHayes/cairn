// E6, H3: entity keys and emails as the screens send them, a merge naming every journey that
// refers to either entity at the revision seen, aliases resolving to the survivor, and the
// merge offered for a caller's duplicate entities.
import type { Schema } from "@cairn/client";
import { describe, expect, it } from "vitest";

import { linkable } from "./Identity.tsx";
import { aliasesOf, emailsFrom, entityOf, mergeMutation, newEntityKey, offeredMerge } from "./model.ts";

describe("entities as sent (E6, H3)", () => {
  it.each(["Ann Example", "", "Équipe d'achats", "y".repeat(300)])("mints a key the host accepts from %j", (name) => {
    const key = newEntityKey(name, () => "abc123");
    expect(key).toMatch(/^e_[a-z0-9][a-z0-9_-]*$/);
    expect(key.length).toBeLessThanOrEqual(64);
  });

  it("reads emails trimmed, lower-cased, each once, by comma, space, or line", () => {
    expect(emailsFrom(" Ann@Example.org, ann@example.org\nbob@example.org;  ")).toEqual(["ann@example.org", "bob@example.org"]);
    expect(entityOf("e_a", " Ann ", "")).toEqual({ key: "e_a", name: "Ann" });
  });
});

describe("a merge (E6)", () => {
  it("names each referring journey at the revision it was read at", () => {
    const summary = (id: string, revision: number): Schema<"JourneySummary"> => ({ id, name: id, status: "archived", revision, created_at: "2026-10-01T00:00:00Z", upgrade_available: false });
    expect(mergeMutation("e_keep", "e_old", [summary("j_one", 4), summary("j_two", 9)])).toEqual({
      op: "merge_entities",
      survivor: "e_keep",
      merged: "e_old",
      journeys: { j_one: 4, j_two: 9 },
    });
  });

  it("lists the old keys resolving to an entity", () => {
    const deployment = { revision: 3, aliases: { e_old: "e_keep", e_older: "e_keep", e_else: "e_other" } };
    expect(aliasesOf(deployment, "e_keep")).toEqual(["e_old", "e_older"]);
    expect(aliasesOf(deployment, "e_none")).toEqual([]);
  });

  it("offers one merge at a time for a caller's duplicates, and none for one entity (H3)", () => {
    expect(offeredMerge(["e_b", "e_a", "e_c"])).toEqual({ survivor: "e_a", merged: "e_b" });
    expect(offeredMerge(["e_a"])).toBeUndefined();
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
