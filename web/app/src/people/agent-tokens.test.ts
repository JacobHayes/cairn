// The Connect an agent page's token machine: one mint at a time, a secret held only while shown,
// and a list that is read again for everything that may have changed it.
import { createActor } from "xstate";
import { describe, expect, it } from "vitest";

import type { AgentTokenHost, MintedToken } from "../data/host.ts";
import { agentTokens } from "./agent-tokens.ts";

/** A promise the test settles when it chooses. */
function pending<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

const settled = () => new Promise((done) => setTimeout(done, 0));

function setup() {
  const mints: { name: string; answer: ReturnType<typeof pending<MintedToken>> }[] = [];
  const revokes: { agent: string; answer: ReturnType<typeof pending<undefined>> }[] = [];
  const tokens: AgentTokenHost = {
    list: () => Promise.resolve([]),
    mint: (name) => {
      const answer = pending<MintedToken>();
      mints.push({ name, answer });
      return answer.promise;
    },
    revoke: (agent) => {
      const answer = pending<undefined>();
      revokes.push({ agent, answer });
      return answer.promise;
    },
  };
  const actor = createActor(agentTokens, { input: { tokens } }).start();
  const name = (typed: string) => {
    actor.send({ type: "open" });
    actor.send({ type: "name", name: typed });
  };
  return { actor, mints, revokes, name, snapshot: () => actor.getSnapshot() };
}

const MINTED: MintedToken = { agent: "ag_one", token: "cairn_agent_secret" };

describe("making a token", () => {
  it("sends one request however many times Make token is pressed, and shows the secret once it answers", async () => {
    const { actor, mints, name, snapshot } = setup();
    name("nightly");
    actor.send({ type: "make" });
    actor.send({ type: "make" });
    expect(mints.map((mint) => mint.name)).toEqual(["nightly"]);
    mints[0]?.answer.resolve(MINTED);
    await settled();
    expect(snapshot().context.minted).toEqual({ name: "nightly", token: MINTED });
    expect(snapshot().context.listVersion).toBe(1);
  });

  it("makes nothing from a blank name", () => {
    const { actor, mints, name } = setup();
    name("   ");
    actor.send({ type: "make" });
    expect(mints).toEqual([]);
  });

  it("drops the secret when it is hidden", async () => {
    const { actor, mints, name, snapshot } = setup();
    name("nightly");
    actor.send({ type: "make" });
    mints[0]?.answer.resolve(MINTED);
    await settled();
    actor.send({ type: "hide" });
    expect(snapshot().context.minted).toBeUndefined();
    expect(snapshot().matches({ mint: "idle" })).toBe(true);
  });

  it("ignores Cancel once the call is out, so the token it makes is never made unseen", async () => {
    const { actor, mints, name, snapshot } = setup();
    name("nightly");
    actor.send({ type: "make" });
    actor.send({ type: "cancel" });
    expect(snapshot().matches({ mint: "minting" })).toBe(true);
    mints[0]?.answer.resolve(MINTED);
    await settled();
    expect(snapshot().context.minted?.token).toEqual(MINTED);
  });

  it("keeps the name typed after a failed mint and asks for the list, in case it was made", async () => {
    const { actor, mints, name, snapshot } = setup();
    name("nightly");
    actor.send({ type: "make" });
    mints[0]?.answer.reject(new Error("offline"));
    await settled();
    expect(snapshot().matches({ mint: "naming" })).toBe(true);
    expect(snapshot().context.name).toBe("nightly");
    expect(snapshot().context.problem).toBeDefined();
    expect(snapshot().context.listVersion).toBe(1);
  });
});

describe("revoking a token", () => {
  it("revokes one at a time, announces it, and asks for the list", async () => {
    const { actor, revokes, snapshot } = setup();
    const announced: string[] = [];
    actor.on("revoked", ({ name }) => {
      announced.push(name);
    });
    actor.send({ type: "revoke", agent: "ag_one", name: "Codex" });
    actor.send({ type: "revoke", agent: "ag_two", name: "Claude Code" });
    expect(revokes.map((revoke) => revoke.agent)).toEqual(["ag_one"]);
    revokes[0]?.answer.resolve(undefined);
    await settled();
    expect(announced).toEqual(["Codex"]);
    expect(snapshot().context.listVersion).toBe(1);
    expect(snapshot().context.revoking).toBeUndefined();
  });

  it("drops the secret on show when it is that token that is revoked, and keeps it for another", async () => {
    const { actor, mints, revokes, name, snapshot } = setup();
    name("nightly");
    actor.send({ type: "make" });
    mints[0]?.answer.resolve(MINTED);
    await settled();
    actor.send({ type: "revoke", agent: "ag_other", name: "Codex" });
    revokes[0]?.answer.resolve(undefined);
    await settled();
    expect(snapshot().context.minted?.token).toEqual(MINTED);
    actor.send({ type: "revoke", agent: MINTED.agent, name: "nightly" });
    revokes[1]?.answer.resolve(undefined);
    await settled();
    expect(snapshot().context.minted).toBeUndefined();
    expect(snapshot().matches({ mint: "idle" })).toBe(true);
  });

  it("says so when a revoke fails, announces nothing, and asks for the list", async () => {
    const { actor, revokes, snapshot } = setup();
    const announced: string[] = [];
    actor.on("revoked", ({ name }) => {
      announced.push(name);
    });
    actor.send({ type: "revoke", agent: "ag_one", name: "Codex" });
    revokes[0]?.answer.reject(new Error("offline"));
    await settled();
    expect(announced).toEqual([]);
    expect(snapshot().context.problem).toBeDefined();
    expect(snapshot().context.listVersion).toBe(1);
  });
});
