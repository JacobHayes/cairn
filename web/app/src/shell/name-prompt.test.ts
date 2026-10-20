// H3: the name prompt asks for the caller's first identity only when it has no name.
import { describe, expect, it } from "vitest";

import type { Caller, Deployment } from "../data/host.ts";
import { asksName } from "./name-prompt.ts";

const caller = (...identities: string[]) => ({ user: "u_1", identities, logins: [] }) as Caller;
const deployment = (...identities: object[]) => ({ revision: 1, identities }) as unknown as Deployment;

describe("when Cairn asks what to call the caller (H3)", () => {
  it("asks for the first identity by key when it has no name", () => {
    const held = deployment({ key: "who_b", name: "Bea" }, { key: "who_a", emails: ["a@example.org"] });
    expect(asksName(caller("who_b", "who_a"), held)?.key).toBe("who_a");
  });

  it("does not ask once the identity is named, or while the caller or deployment is unread", () => {
    const named = deployment({ key: "who_a", name: "Ana", emails: ["a@example.org"] });
    expect(asksName(caller("who_a"), named)).toBeUndefined();
    expect(asksName(undefined, named)).toBeUndefined();
    expect(asksName(caller("who_a"), undefined)).toBeUndefined();
  });

  it("does not ask when the caller holds no identity in the deployment", () => {
    expect(asksName(caller(), deployment({ key: "who_a" }))).toBeUndefined();
    expect(asksName(caller("who_z"), deployment({ key: "who_a" }))).toBeUndefined();
  });
});
