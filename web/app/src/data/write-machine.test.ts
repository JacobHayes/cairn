// H5: a write is delivered once however many times it is sent, and resent with its patch id
// until it is answered. The write's states, its resends (the backoff, the wait for the
// connection, the limit), and what each answer makes of it (A18, A19).
import { backoffDelayMs, type Answered, type HttpFailure, type Patch, type PatchAnswer, type Rejection } from "@cairn/client";
import { HostFailure } from "@cairn/wasm";
import { SimulatedClock, createActor } from "xstate";
import { describe, expect, it } from "vitest";

import type { Command } from "./edit-command.ts";
import { hostFailure } from "./host-failure.ts";
import { FakeHost, deferred, settled, writeEnv } from "./fake.test-support.ts";
import { RESEND_COUNT_MAX, writeMachine, writeView, type WriteView } from "./write-machine.ts";
import { patchOf } from "./writes.ts";

const receipt = { patch_id: "p_x", domain: { journey: "j_one" }, content_hash: "h", revision: 4 };
const applied: Answered<HttpFailure> = { outcome: "answered", answer: { outcome: "applied", receipt, consequences: {} } };
const alreadyApplied: Answered<HttpFailure> = { outcome: "answered", answer: { outcome: "already_applied", receipt } as PatchAnswer };
const lost: Answered<HttpFailure> = { outcome: "failed", error: { status: 0, message: "network down" } };
const busy: Answered<HttpFailure> = { outcome: "failed", error: { status: 503, message: "busy" } };
const unauthorized: Answered<HttpFailure> = { outcome: "failed", error: { status: 401, message: "sign in" } };
const stale = (current: number): Answered<HttpFailure> => ({
  outcome: "rejected",
  rejection: { rejection: "stale", conflicts: [{ of: { domain: { journey: "j_one" } }, expected: 3, current }], intervening: [], touched: [] },
});

const patch: Patch = patchOf({ target: { journey: "j_one" }, baseRevision: 3, mutations: [{ op: "set_node_field", node: "n_a", value: { title: "Renamed" } }] }, "p_one");
const about = { edit: "title:j_one:n_a", label: "Draft the plan", address: "/journeys/j_one/n_a" };

/** A write over a fake host and a simulated clock, . */
function setup(answers: Answered<HttpFailure>[] = [], command: Command = { kind: "patch", patch }) {
  const host = new FakeHost();
  host.answers = answers;
  const { env, switches, saves } = writeEnv(host);
  const clock = new SimulatedClock();
  const actor = createActor(writeMachine, { input: { id: patch.id, command, about, env }, clock });
  actor.start();
  const view = (): WriteView => writeView(actor.getSnapshot());
  /** Advances the clock, then lets the answers that came back run. */
  const advance = async (ms: number) => {
    clock.increment(ms);
    await settled();
  };
  return { host, switches, saves, actor, view, advance };
}

describe("a write's delivery", () => {
  it("sends once however many times it is submitted", async () => {
    const { host, actor, view } = setup();
    host.answers = [applied];
    actor.send({ type: "submit" });
    actor.send({ type: "submit" });
    await settled();
    expect(host.sent).toHaveLength(1);
    expect(view().state).toBe("landed");
  });

  it("refuses a second submit while the first is out, not only after it was answered", async () => {
    const { host, actor, view } = setup();
    const held = deferred<Answered<HttpFailure>>();
    host.answers = [held.promise];
    actor.send({ type: "submit" });
    await settled();
    actor.send({ type: "submit" });
    await settled();
    expect(host.sent).toHaveLength(1);
    expect(view().state).toBe("sending");
    held.resolve(applied);
    await settled();
    expect(view().state).toBe("landed");
  });

  it("sends a patch as it is: a stale answer is a rejection, never resent rebased", async () => {
    const { host, saves, actor, view } = setup();
    host.answers = [stale(5), applied];
    actor.send({ type: "submit" });
    await settled();
    expect(host.sent.map((sent) => [sent.id, sent.base_revision])).toEqual([["p_one", 3]]);
    expect(view()).toMatchObject({ state: "rejected", rejection: { rejection: "stale" } });
    expect(saves).toHaveLength(0);
  });

  it("resends a lost write with its id and content after the first wait, and settles on already_applied", async () => {
    const { host, saves, actor, view, advance } = setup();
    host.answers = [lost, alreadyApplied];
    actor.send({ type: "submit" });
    await settled();
    expect(view()).toMatchObject({ state: "unconfirmed", retrying: true, failure: { status: 0 } });
    await advance(backoffDelayMs(0) - 1);
    expect(host.sent).toHaveLength(1);
    await advance(1);
    expect(host.sent).toHaveLength(2);
    expect(host.sent[1]).toEqual(host.sent[0]);
    expect(view().state).toBe("landed");
    expect(saves).toHaveLength(1);
  });
});

describe("a write that got no answer", () => {
  it("backs off between resends, then stops after the limit and offers Retry by hand", async () => {
    const { host, actor, view, advance } = setup();
    host.answers = Array.from({ length: RESEND_COUNT_MAX + 3 }, () => lost);
    actor.send({ type: "submit" });
    await settled();
    for (let resend = 0; resend < RESEND_COUNT_MAX; resend += 1) {
      expect(view()).toMatchObject({ state: "unconfirmed", retrying: true });
      await advance(backoffDelayMs(resend) - 1);
      expect(host.sent).toHaveLength(1 + resend);
      await advance(1);
      expect(host.sent).toHaveLength(2 + resend);
    }
    expect(view()).toMatchObject({ state: "unconfirmed", retrying: false });
    await advance(60_000);
    expect(host.sent).toHaveLength(1 + RESEND_COUNT_MAX);
    actor.send({ type: "retry" });
    await settled();
    expect(host.sent).toHaveLength(2 + RESEND_COUNT_MAX);
  });
});

describe("a write waiting for the connection", () => {
  it("sends nothing offline, spends no attempt, and resends when the connection is back", async () => {
    const { host, switches, actor, view, advance } = setup();
    switches.connected = false;
    host.answers = [lost, ...Array.from({ length: RESEND_COUNT_MAX }, () => lost)];
    actor.send({ type: "submit" });
    await settled();
    await advance(60_000);
    expect(host.sent).toHaveLength(1);
    expect(view()).toMatchObject({ state: "unconfirmed", retrying: true });
    switches.connected = true;
    actor.send({ type: "wake" });
    await settled();
    expect(host.sent).toHaveLength(2);
    // The whole budget is still there: five resends in all, after the minute spent offline.
    for (let resend = 1; resend < RESEND_COUNT_MAX; resend += 1) {
      await advance(backoffDelayMs(resend));
    }
    expect(host.sent).toHaveLength(1 + RESEND_COUNT_MAX);
    expect(view()).toMatchObject({ state: "unconfirmed", retrying: false });
  });
});

describe("what an answer makes of a write", () => {
  it("stops a write the in-browser root refused, as it stops one the server refused", async () => {
    const { host, actor, view, advance } = setup();
    const refusal = hostFailure(new HostFailure({ error: "missing", message: "no such journey" }));
    host.answers = [{ outcome: "failed", error: refusal }, applied];
    actor.send({ type: "submit" });
    await settled();
    expect(view()).toMatchObject({ state: "stopped", failure: { status: 404, message: "no such journey" } });
    await advance(60_000);
    expect(host.sent).toHaveLength(1);
  });

  it("leaves a throw that is not a refusal as no answer, to be resent", async () => {
    const { host, actor, view } = setup();
    host.answers = [{ outcome: "failed", error: hostFailure(new Error("worker died")) }, applied];
    actor.send({ type: "submit" });
    await settled();
    expect(view().state).toBe("unconfirmed");
  });

  it("makes a 4xx answer to the first send a stop, not an uncertainty", async () => {
    const { host, actor, view, advance } = setup();
    host.answers = [unauthorized];
    actor.send({ type: "submit" });
    await settled();
    expect(view()).toMatchObject({ state: "stopped", failure: { status: 401 } });
    await advance(60_000);
    expect(host.sent).toHaveLength(1);
  });

  it("makes a 5xx answer an uncertainty", async () => {
    const { host, actor, view } = setup();
    host.answers = [busy];
    actor.send({ type: "submit" });
    await settled();
    expect(view()).toMatchObject({ state: "unconfirmed", retrying: true });
  });

  it("keeps a write unconfirmed when its resend is refused, since the lost attempt may have committed", async () => {
    const { host, actor, view, advance } = setup();
    host.answers = [lost, unauthorized];
    actor.send({ type: "submit" });
    await settled();
    await advance(backoffDelayMs(0));
    expect(view()).toMatchObject({ state: "unconfirmed", retrying: false, failure: { status: 401 } });
  });

  it("keeps a rejection with what it tried to send, until it is discarded", async () => {
    const { host, actor, view } = setup();
    host.answers = [{ outcome: "rejected", rejection: { rejection: "invalid", violations: [] } }];
    actor.send({ type: "submit" });
    await settled();
    expect(view()).toMatchObject({ state: "rejected", command: { patch } });
    actor.send({ type: "submit" });
    await settled();
    expect(host.sent).toHaveLength(1);
    actor.send({ type: "discard" });
    expect(actor.getSnapshot().status).toBe("done");
  });
});

describe("version skew", () => {
  it("sends nothing and stops the write before dispatch", async () => {
    const { host, switches, actor, view } = setup();
    switches.compatible = false;
    actor.send({ type: "submit" });
    await settled();
    expect(host.sent).toEqual([]);
    expect(view()).toMatchObject({ state: "stopped", failure: undefined });
    actor.send({ type: "retry" });
    await settled();
    expect(view().state).toBe("stopped");
    switches.compatible = true;
    host.answers = [applied];
    actor.send({ type: "retry" });
    await settled();
    expect(view().state).toBe("landed");
  });

  it("gives up waiting at once, without a resend", async () => {
    const waiting = setup();
    waiting.switches.connected = false;
    waiting.host.answers = [lost];
    waiting.actor.send({ type: "submit" });
    await settled();
    await waiting.advance(backoffDelayMs(0));
    waiting.switches.compatible = false;
    waiting.actor.send({ type: "wake" });
    expect(waiting.view()).toMatchObject({ state: "unconfirmed", retrying: false });
  });
});

describe("shutdown", () => {
  it("leaves no timer behind: a write in backoff does not resend after it is stopped", async () => {
    const { host, actor, advance } = setup();
    host.answers = [lost, applied];
    actor.send({ type: "submit" });
    await settled();
    actor.stop();
    await advance(60_000);
    expect(host.sent).toHaveLength(1);
  });
});

const applyCommand: Command = {
  kind: "proposal",
  call: { op: "apply", destination: { journey: "j_one" }, proposal: "pr_one", request: { patch_id: "p_one", reviewed_revision: 4 } },
};
const createCommand: Command = {
  kind: "proposal",
  call: { op: "create", destination: { journey: "j_one" }, request: { id: "pr_one", patch_id: "p_one", draft: { mutations: [], base_revision: 3 } as never } },
};
const importCommand: Command = {
  kind: "import",
  request: { patch_id: "p_one", file: { format: 1, route: "r_one", name: "One", nodes: [] } as never },
};
const saved = { outcome: "already_saved" } as never;
const proposalLanded = { outcome: "answered", answer: saved } as const;

describe("a proposal write or an import", () => {
  it("is sent once: a stale answer is a rejection, for a person to review again", async () => {
    const { host, actor, view } = setup([], applyCommand);
    host.proposed = [{ outcome: "rejected", rejection: { rejection: "stale", conflicts: [], intervening: [], touched: [] } }];
    actor.send({ type: "submit" });
    await settled();
    expect(host.called).toEqual(["apply"]);
    expect(view()).toMatchObject({ state: "rejected", rejection: { rejection: "stale" } });
    actor.send({ type: "submit" });
    await settled();
    expect(host.called).toEqual(["apply"]);
  });

  it("is resent as the same request when its answer was lost, and settles on the answer it gets then", async () => {
    const { host, actor, view, advance } = setup([], createCommand);
    host.proposed = [lost, proposalLanded];
    actor.send({ type: "submit" });
    await settled();
    expect(view()).toMatchObject({ state: "unconfirmed", retrying: true });
    await advance(backoffDelayMs(0));
    expect(host.called).toEqual(["create", "create"]);
    expect(view()).toMatchObject({ state: "landed" });
  });

  it("sends an import once, resends it when lost, and stops on a refusal before anything committed", async () => {
    const resent = setup([], importCommand);
    resent.host.imports = [lost, alreadyApplied];
    resent.actor.send({ type: "submit" });
    await settled();
    await resent.advance(backoffDelayMs(0));
    expect(resent.host.imported.map((each) => each.patch_id)).toEqual(["p_one", "p_one"]);
    expect(resent.view().state).toBe("landed");

    const refused = setup([], importCommand);
    refused.host.imports = [unauthorized];
    refused.actor.send({ type: "submit" });
    await settled();
    expect(refused.view()).toMatchObject({ state: "stopped", failure: { status: 401 } });
    await refused.advance(60_000);
    expect(refused.host.imported).toHaveLength(1);
  });

  it("is not a patch: it sends nothing through the patch call and keeps what it was asked", async () => {
    const { host, actor, view } = setup([], importCommand);
    host.imports = [{ outcome: "rejected", rejection: { rejection: "invalid", violations: [] } }];
    actor.send({ type: "submit" });
    await settled();
    expect(host.sent).toEqual([]);
    expect(view()).toMatchObject({ state: "rejected", command: importCommand });
  });
});

describe("a check before the first send", () => {
  const refusal: Rejection = { rejection: "invalid", violations: [] };

  it("refuses the write without sending anything, and keeps the refusal with the command", async () => {
    const { host, actor, view } = setup();
    actor.send({ type: "submit", check: () => Promise.resolve(refusal) });
    await settled();
    expect(host.sent).toEqual([]);
    expect(view()).toMatchObject({ state: "rejected", rejection: refusal, command: { patch } });
  });

  it("says the subject is gone when the check's refusal is the shape of a missing one", async () => {
    const { actor, view } = setup();
    const missing: Rejection = { rejection: "stale", conflicts: [{ of: { domain: { journey: "j_one" } }, expected: 3, current: 0 }], intervening: [], touched: [] };
    actor.send({ type: "submit", check: () => Promise.resolve(missing) });
    await settled();
    expect(view()).toMatchObject({ state: "gone", removed: { journey: "j_one" } });
  });

  it.each([
    ["passes", () => Promise.resolve(undefined)],
    ["throws", () => Promise.reject(new Error("the engine is not there"))],
  ])("sends the write once when the check %s", async (_, check) => {
    const { host, actor, view } = setup();
    host.answers = [applied];
    actor.send({ type: "submit", check });
    await settled();
    expect(host.sent).toHaveLength(1);
    expect(view().state).toBe("landed");
  });

  it("is busy while it runs and takes no second submit, which would be a second check", async () => {
    const { host, actor, view } = setup();
    const held = deferred<Rejection | undefined>();
    let checks = 0;
    const check = () => {
      checks += 1;
      return held.promise;
    };
    host.answers = [applied];
    actor.send({ type: "submit", check });
    actor.send({ type: "submit", check });
    await settled();
    expect(view().state).toBe("sending");
    held.resolve(undefined);
    await settled();
    expect(checks).toBe(1);
    expect(host.sent).toHaveLength(1);
  });

});

describe("a check that is overtaken", () => {
  it("stops the write, sending nothing, when the tab went into skew while it ran", async () => {
    const { host, switches, actor, view } = setup();
    const held = deferred<Rejection | undefined>();
    actor.send({ type: "submit", check: () => held.promise });
    await settled();
    switches.compatible = false;
    held.resolve(undefined);
    await settled();
    expect(host.sent).toEqual([]);
    expect(view()).toMatchObject({ state: "stopped" });
  });

  it("is not run again by a Retry of a write it let through", async () => {
    const { host, actor, view } = setup();
    let checks = 0;
    host.answers = [{ outcome: "failed", error: { status: 401, message: "sign in" } }, applied];
    actor.send({ type: "submit", check: () => {
      checks += 1;
      return Promise.resolve(undefined);
    } });
    await settled();
    expect(view().state).toBe("stopped");
    actor.send({ type: "retry" });
    await settled();
    expect(checks).toBe(1);
    expect(view().state).toBe("landed");
  });
});
