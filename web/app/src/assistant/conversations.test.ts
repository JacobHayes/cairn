// The session's conversations (I5): one per assistant target, started when a panel first opens
// it and kept for the tab, so a turn answered after the panel moved on still lands in its own.
import { createActor } from "xstate";
import { describe, expect, it } from "vitest";

import { memoryTurns, settled } from "../data/fake.test-support.ts";
import { conversationView, sendTurn } from "./conversation.ts";
import { conversationManager } from "./conversations.ts";
import { shown } from "./model.ts";
import { HandHost, answered, holding, replied, said, target } from "./hand-host.test-support.ts";

const other = { route: "r_one" };

function manager(offered = true) {
  const host = new HandHost();
  const ref = createActor(conversationManager, { input: { assistant: offered ? host : undefined, turns: memoryTurns() } }).start();
  const held = (key: string) => ref.getSnapshot().context.conversations[key];
  return { host, ref, held };
}

describe("the conversation manager", () => {
  it("holds one conversation per target, read again each time a panel opens it", () => {
    const { host, ref, held } = manager();
    ref.send({ type: "open", target });
    const first = held("journey:j_one");
    expect(first).toBeDefined();
    expect(host.readsOf(target)).toBe(1);
    ref.send({ type: "open", target });
    expect(held("journey:j_one")).toBe(first);
    expect(host.readsOf(target)).toBe(2);
    ref.send({ type: "open", target: other });
    expect(held("route:r_one")).not.toBe(first);
    expect(Object.keys(ref.getSnapshot().context.conversations)).toEqual(["journey:j_one", "route:r_one"]);
  });

  it("lands a turn answered after the panel moved to another target in its own conversation only", async () => {
    const { host, ref, held } = manager();
    ref.send({ type: "open", target });
    const first = held("journey:j_one");
    if (first === undefined) {
      throw new Error("the conversation was not started");
    }
    const sent = sendTurn(first, "Hello?");
    ref.send({ type: "open", target: other });
    const second = held("route:r_one");
    host.turns[0]?.answer(replied("Hi."));
    await settled();
    host.reads.filter((each) => each.target === target).at(-1)?.answer(holding([said("Hello?", "t1"), answered("Hi.", "t2")]));
    await sent;
    const words = (view: ReturnType<typeof conversationView>) => {
      const messages = view.read.status === "ready" ? view.read.conversation.messages : [];
      return shown(messages, view.kept, view.unsaved).flatMap((entry) => (entry.entry === "said" || entry.entry === "reply" ? [entry.text] : []));
    };
    expect(words(conversationView(first.getSnapshot()))).toEqual(["Hello?", "Hi."]);
    // The other target's conversation heard nothing of it.
    expect(second && conversationView(second.getSnapshot())).toMatchObject({ read: { status: "loading" }, sending: undefined, kept: [], unsaved: [] });
    // Opening the first again shows the answer, which the host kept.
    ref.send({ type: "open", target });
    expect(words(conversationView(first.getSnapshot()))).toEqual(["Hello?", "Hi."]);
  });

  it("starts nothing without an assistant", () => {
    const { host, ref } = manager(false);
    ref.send({ type: "open", target });
    expect(ref.getSnapshot().context.conversations).toEqual({});
    expect(host.reads).toEqual([]);
  });
});
