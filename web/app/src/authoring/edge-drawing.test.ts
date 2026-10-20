// A3, H5: drawing an edge is a machine over the real edit manager. It sends its pick as a
// write and then only watches the write by its id, so every way of stopping the drawing is one
// reset that leaves the write alone, a busy drawing hears no second pick, and an earlier
// write's answer can never close a later drawing. The path tests walk the machine's own
// specification with @xstate/graph; the actor tests check delivery and cleanup.
import type { Answered, HttpFailure, Rejection } from "@cairn/client";
import { getAdjacencyMap, getShortestPaths } from "@xstate/graph";
import { createActor, initialTransition, type EventFromLogic } from "xstate";
import { describe, expect, it } from "vitest";

import { RESEND_COUNT_MAX, writeView } from "../data/write-machine.ts";
import { problemsOf } from "../data/edit-manager.ts";
import type { EditsRef } from "../data/session-root.ts";
import { startRoot } from "../data/edits.test-support.ts";
import { FakeHost, deferred, settled } from "../data/fake.test-support.ts";
import { drawingView, edgeDrawing, edgeEdit, type DrawingEvent, type DrawingInput, type DrawingSnapshot, type KeySource } from "./edge-drawing.ts";
import { treeOf, type Graph } from "./graph.ts";
import type { Authored } from "./target.ts";
import type { Preview } from "./write.ts";

// A group holding n_a, and three top-level nodes.
const graph: Graph = {
  nodes: [
    { key: "n_group", id: "group", kind: "group", title: "Group" },
    { key: "n_a", id: "a", kind: "deliverable", title: "A", parent: "n_group" },
    { key: "n_b", id: "b", kind: "deliverable", title: "B" },
    { key: "n_c", id: "c", kind: "deliverable", title: "C" },
    { key: "n_d", id: "d", kind: "deliverable", title: "D" },
  ],
};

const authored: Authored = {
  target: { route: "r_one" },
  graph,
  tree: treeOf(graph),
  revision: 3,
  deploymentRevision: 1,
  deployment: { revision: 1 },
  today: "2026-10-06",
  lineage: undefined,
  route: undefined,
};

const receipt = { patch_id: "p_x", domain: { route: "r_one" }, content_hash: "h", revision: 4 };
const applied: Answered<HttpFailure> = { outcome: "answered", answer: { outcome: "applied", receipt, consequences: {} } };
const lost: Answered<HttpFailure> = { outcome: "failed", error: { status: 0, message: "network down" } };
const refusedByHost: Answered<HttpFailure> = { outcome: "failed", error: { status: 400, message: "not acceptable" } };
const invalid: Rejection = { rejection: "invalid", violations: [{ at: {}, code: "dependency_cycle", message: "That would make a cycle" }] };
const accepted: Preview = { outcome: "accepted", consequences: undefined };
const rejected: Preview = { outcome: "rejected", rejection: invalid };

/** Escape, as the page hears it: the drawing listens while it is active and stops when it is not. */
function fakeKeys() {
  const cancels = new Set<() => void>();
  const source: KeySource = {
    listen: (cancel) => {
      cancels.add(cancel);
      return () => {
        cancels.delete(cancel);
      };
    },
  };
  return {
    source,
    get listening() {
      return cancels.size;
    },
    press: () => {
      for (const cancel of [...cancels]) {
        cancel();
      }
    },
  };
}

/** A drawing over a real edit manager and a fake host; the previews wait for the test. */
async function drawing(options: { host?: FakeHost } = {}) {
  const root = await startRoot(options);
  const keys = fakeKeys();
  const previews: { mutations: unknown[]; answer: ReturnType<typeof deferred<Preview>> }[] = [];
  const flags = { compatible: true };
  const input: DrawingInput = {
    edits: root.edits,
    preview: (_, mutations) => {
      const answer = deferred<Preview>();
      previews.push({ mutations, answer });
      return answer.promise;
    },
    compatible: () => flags.compatible,
    keys: keys.source,
  };
  const actor = createActor(edgeDrawing, { input });
  actor.start();
  const view = () => drawingView(actor.getSnapshot());
  return {
    ...root,
    actor,
    keys,
    previews,
    flags,
    view,
    start: (from: string) => {
      actor.send({ type: "start", from });
    },
    pick: (to: string) => {
      actor.send({ type: "pick", to, authored, user: "u_fake", address: "/routes/r_one/draft" });
    },
    cancel: () => {
      actor.send({ type: "cancel" });
    },
    /** Answers the oldest unanswered preview. */
    answerPreview: async (answer: Preview, at = 0) => {
      previews[at]?.answer.resolve(answer);
      await settled();
    },
    /** The id of the write the drawing is showing. */
    sentId: () => {
      const shown = view();
      if (shown.state !== "saving" && !(shown.state === "choosing" && shown.sent !== undefined)) {
        throw new Error(`no write was sent: ${shown.state}`);
      }
      return shown.state === "saving" ? shown.sent.id : (shown.sent?.id ?? "");
    },
  };
}

const held = () => deferred<Answered<HttpFailure>>();

describe("every way of stopping the drawing is one reset (A3)", () => {
  const ways = {
    "the button": async (d: Awaited<ReturnType<typeof drawing>>) => {
      d.host.answers = [refusedByHost];
      await dirty(d);
      d.cancel();
    },
    Escape: async (d: Awaited<ReturnType<typeof drawing>>) => {
      d.host.answers = [refusedByHost];
      await dirty(d);
      d.keys.press();
    },
    "the write landing": async (d: Awaited<ReturnType<typeof drawing>>) => {
      d.host.answers = [refusedByHost, applied];
      await dirty(d);
      d.edits.send({ type: "retry", id: d.sentId() });
      await settled();
    },
  };

  /** A drawing with something of everything in it: a refusal, then a pick whose write the host refused. */
  async function dirty(d: Awaited<ReturnType<typeof drawing>>) {
    d.start("n_a");
    d.pick("n_group");
    expect(d.actor.getSnapshot().context.refused?.by).toBe("containment");
    d.pick("n_d");
    await d.answerPreview(accepted);
    expect(d.view()).toMatchObject({ state: "choosing", sent: { to: "n_d" } });
  }

  it.each(Object.entries(ways))("%s leaves a drawing nothing is left of, hearing no key", async (_, stop) => {
    const d = await drawing();
    await stop(d);
    expect(d.view()).toEqual({ state: "inactive" });
    expect(d.actor.getSnapshot().context).toMatchObject({ from: undefined, refused: undefined, picked: undefined, sent: undefined });
    expect(d.keys.listening).toBe(0);
  });

  it("leaves the write with the edit manager when the drawing stops", async () => {
    const d = await drawing();
    d.host.answers = [refusedByHost];
    await dirty(d);
    const id = d.sentId();
    d.keys.press();
    expect(writeView(d.writes()[id]?.snapshot ?? failed("the write is gone"))).toMatchObject({ state: "stopped" });
  });
});

function failed(message: string): never {
  throw new Error(message);
}

describe("a pick while the drawing is busy is not heard (H5)", () => {
  it("sends one write for two quick picks, and none for a pick while it is saving", async () => {
    const d = await drawing();
    const answer = held();
    d.host.answers = [answer.promise];
    d.start("n_a");
    d.pick("n_c");
    d.pick("n_d");
    expect(d.previews).toHaveLength(1);
    await d.answerPreview(accepted);
    expect(d.view()).toMatchObject({ state: "saving", from: "n_a", to: "n_c" });
    d.pick("n_d");
    d.start("n_b");
    await settled();
    expect(d.view()).toMatchObject({ state: "saving", from: "n_a", to: "n_c" });
    expect(d.previews).toHaveLength(1);
    expect(d.host.sent.map((patch) => patch.mutations)).toEqual([[{ op: "add_edge", edge: { node: "n_a", requires: "n_c" } }]]);
    answer.resolve(applied);
    await settled();
    expect(d.view()).toEqual({ state: "inactive" });
  });
});

describe("an earlier write can never close a later drawing (H5)", () => {
  it("leaves the second drawing alone when the first, cancelled one is answered, and keeps the first write", async () => {
    const d = await drawing();
    const [first, second] = [held(), held()];
    d.host.answers = [first.promise, second.promise];
    d.start("n_a");
    d.pick("n_c");
    await d.answerPreview(accepted, 0);
    const firstId = d.sentId();
    d.cancel();
    expect(Object.keys(d.writes())).toEqual([firstId]);
    d.start("n_b");
    d.pick("n_d");
    await d.answerPreview(accepted, 1);
    const secondId = d.sentId();
    expect(secondId).not.toBe(firstId);
    first.resolve(applied);
    await settled();
    expect(d.view()).toMatchObject({ state: "saving", from: "n_b", to: "n_d", sent: { id: secondId } });
    expect(Object.keys(d.writes())).toEqual([secondId]);
    expect(d.saves).toHaveLength(1);
    second.resolve(applied);
    await settled();
    expect(d.view()).toEqual({ state: "inactive" });
    expect(d.saves).toHaveLength(2);
  });

  it("leaves the second drawing saving when the first write is refused, and the first on the chip", async () => {
    const d = await drawing();
    const [first, second] = [held(), held()];
    d.host.answers = [first.promise, second.promise];
    d.start("n_a");
    d.pick("n_c");
    await d.answerPreview(accepted, 0);
    const firstId = d.sentId();
    d.cancel();
    d.start("n_b");
    d.pick("n_d");
    await d.answerPreview(accepted, 1);
    first.resolve({ outcome: "rejected", rejection: invalid });
    await settled();
    expect(d.view()).toMatchObject({ state: "saving", from: "n_b", to: "n_d" });
    expect(problemsOf(d.writes(), false).map((problem) => problem.id)).toEqual([firstId]);
  });
});

describe("stopping while the pick is being checked sends nothing", () => {
  it.each(["cancel", "stop the page"] as const)("%s, then the check answers", async (how) => {
    const d = await drawing();
    d.start("n_a");
    d.pick("n_c");
    expect(d.view()).toMatchObject({ state: "checking", to: "n_c" });
    if (how === "cancel") {
      d.cancel();
    } else {
      d.actor.stop();
    }
    await d.answerPreview(accepted);
    expect(d.host.sent).toEqual([]);
    expect(d.writes()).toEqual({});
    expect(d.keys.listening).toBe(0);
    if (how === "cancel") {
      d.start("n_b");
      expect(d.view()).toMatchObject({ state: "choosing", from: "n_b" });
    }
  });
});

describe("a pick the rules refuse stays choosing (A3)", () => {
  it("refuses containment before any check, and a failed check before any send, and a valid pick clears the reason", async () => {
    const d = await drawing();
    d.host.answers = [applied];
    d.start("n_a");
    d.pick("n_group");
    expect(d.view()).toMatchObject({ state: "choosing", from: "n_a", refused: { by: "containment" } });
    expect(d.previews).toHaveLength(0);
    d.pick("n_c");
    await d.answerPreview(rejected, 0);
    expect(d.view()).toMatchObject({ state: "choosing", from: "n_a", refused: { by: "preview", rejection: invalid } });
    expect(d.host.sent).toEqual([]);
    d.pick("n_d");
    expect(d.view()).toMatchObject({ state: "checking", to: "n_d" });
    await d.answerPreview(accepted, 1);
    expect(d.host.sent).toHaveLength(1);
    expect(d.view()).toMatchObject({ state: "inactive" });
  });

  it("still sends the pick when the engine could not say", async () => {
    const d = await drawing();
    d.host.answers = [held().promise];
    d.start("n_a");
    d.pick("n_c");
    await d.answerPreview({ outcome: "unavailable", message: "no engine" });
    expect(d.view()).toMatchObject({ state: "saving" });
    expect(d.host.sent).toHaveLength(1);
  });
});

describe("the drawing follows its write wherever it is dealt with (H5)", () => {
  it("is closed by a Retry from the chip that lands", async () => {
    const d = await drawing();
    d.host.answers = [refusedByHost, held().promise];
    d.start("n_a");
    d.pick("n_c");
    await d.answerPreview(accepted);
    const id = d.sentId();
    expect(d.view()).toMatchObject({ state: "choosing", sent: { id } });
    d.edits.send({ type: "retry", id });
    await settled();
    expect(d.view()).toMatchObject({ state: "saving", sent: { id } });
    expect(d.host.sent).toHaveLength(2);
  });

  it("lets go of a write discarded from the chip, and is ready for another pick", async () => {
    const d = await drawing();
    d.host.answers = [refusedByHost, applied];
    d.start("n_a");
    d.pick("n_c");
    await d.answerPreview(accepted);
    d.edits.send({ type: "discard", id: d.sentId() });
    await settled();
    expect(d.view()).toEqual({ state: "choosing", from: "n_a", refused: undefined, sent: undefined });
    expect(d.writes()).toEqual({});
    d.pick("n_c");
    await d.answerPreview(accepted, 1);
    expect(d.view()).toEqual({ state: "inactive" });
  });

  it("replaces a refused write of the same edge by a new pick", async () => {
    const d = await drawing();
    d.host.answers = [{ outcome: "rejected", rejection: invalid }, applied];
    d.start("n_a");
    d.pick("n_c");
    await d.answerPreview(accepted, 0);
    expect(d.view()).toMatchObject({ state: "choosing", sent: { to: "n_c" } });
    expect(writeView(d.writes()[d.sentId()]?.snapshot ?? failed("gone"))).toMatchObject({ state: "rejected" });
    d.pick("n_c");
    await d.answerPreview(accepted, 1);
    expect(d.view()).toEqual({ state: "inactive" });
    expect(d.writes()).toEqual({});
  });
});

describe("a write with no answer keeps the drawing saving while it is resent (H5)", () => {
  it("closes when a resend lands", async () => {
    const d = await drawing();
    d.host.answers = [lost, applied];
    d.start("n_a");
    d.pick("n_c");
    await d.answerPreview(accepted);
    expect(d.view()).toMatchObject({ state: "saving" });
    d.pick("n_d");
    expect(d.host.sent).toHaveLength(1);
    await d.advance(15_000);
    expect(d.view()).toEqual({ state: "inactive" });
    expect(d.host.sent).toHaveLength(2);
  });

  it("asks the person once the resends run out, and a Retry then lands it", async () => {
    const d = await drawing();
    d.host.answers = Array.from({ length: RESEND_COUNT_MAX + 1 }, () => lost);
    d.start("n_a");
    d.pick("n_c");
    await d.answerPreview(accepted);
    for (let resend = 0; resend < RESEND_COUNT_MAX; resend += 1) {
      expect(d.view()).toMatchObject({ state: "saving" });
      await d.advance(15_000);
    }
    const id = d.sentId();
    expect(d.view()).toMatchObject({ state: "choosing", sent: { id } });
    expect(writeView(d.writes()[id]?.snapshot ?? failed("gone"))).toMatchObject({ state: "unconfirmed", retrying: false });
    d.host.answers = [applied];
    d.edits.send({ type: "retry", id });
    await settled();
    expect(d.view()).toEqual({ state: "inactive" });
  });

  it("waits offline, and closes when the connection comes back and the resend lands", async () => {
    const d = await drawing();
    d.host.answers = [lost];
    d.start("n_a");
    d.pick("n_c");
    await d.answerPreview(accepted);
    d.switches.connected = false;
    await d.advance(15_000);
    expect(d.view()).toMatchObject({ state: "saving" });
    expect(d.host.sent).toHaveLength(1);
    d.switches.connected = true;
    d.host.answers = [applied];
    d.root.send({ type: "wake" });
    await settled();
    expect(d.view()).toEqual({ state: "inactive" });
  });
});

describe("version skew (H5)", () => {
  it("previews nothing, sends nothing, and keeps the pick's write stopped on the manager", async () => {
    const d = await drawing();
    d.root.send({ type: "skew" });
    d.flags.compatible = false;
    await settled();
    d.start("n_a");
    d.pick("n_c");
    await settled();
    expect(d.previews).toHaveLength(0);
    expect(d.host.sent).toEqual([]);
    const id = d.sentId();
    expect(d.view()).toMatchObject({ state: "choosing", sent: { to: "n_c" } });
    expect(writeView(d.writes()[id]?.snapshot ?? failed("gone"))).toMatchObject({ state: "stopped" });
    expect(d.storage.length).toBe(1);
  });
});

describe("the same edge still out", () => {
  it("is refused as busy, not sent twice", async () => {
    const d = await drawing();
    d.host.answers = [held().promise];
    d.start("n_a");
    d.pick("n_c");
    await d.answerPreview(accepted, 0);
    d.cancel();
    d.start("n_a");
    d.pick("n_c");
    await d.answerPreview(accepted, 1);
    expect(d.view()).toMatchObject({ state: "choosing", sent: undefined, refused: { by: "busy" } });
    expect(d.host.sent).toHaveLength(1);
  });
});

describe("Escape is heard only while drawing", () => {
  it("listens from the start to every way out, and a press while not drawing changes nothing", async () => {
    const d = await drawing();
    expect(d.keys.listening).toBe(0);
    d.keys.press();
    expect(d.view()).toEqual({ state: "inactive" });
    d.start("n_a");
    expect(d.keys.listening).toBe(1);
    d.start("n_b");
    expect(d.keys.listening).toBe(1);
    expect(d.view()).toMatchObject({ state: "choosing", from: "n_b" });
    d.cancel();
    expect(d.keys.listening).toBe(0);
  });
});

describe("the edge is the edit, wherever it is drawn (H5)", () => {
  it("names the domain and both ends", () => {
    expect(new Set([edgeEdit(authored, "n_a", "n_c"), edgeEdit(authored, "n_a", "n_d"), edgeEdit(authored, "n_b", "n_c"), edgeEdit({ target: { journey: "j_one" } }, "n_a", "n_c")]).size).toBe(4);
  });
});

// The machine's own specification, walked with @xstate/graph. A walk runs no custom action and
// starts no child, so it checks what the machine says; the actor tests above check delivery.
type Event = EventFromLogic<typeof edgeDrawing>;
type Step = { event: Event; state: DrawingSnapshot };
interface Walk {
  /** Every reachable state, with each event it hears and where it goes. */
  adjacency: { state: DrawingSnapshot; steps: Step[] }[];
  reached: DrawingSnapshot[];
}

const walkKeys: KeySource = { listen: () => () => undefined };
const walkInput = (compatible: boolean): DrawingInput => ({ edits: {} as EditsRef, preview: () => new Promise<Preview>(() => undefined), compatible: () => compatible, keys: walkKeys });
const picks = (["n_c", "n_d", "n_group"] as const).map((to): DrawingEvent => ({ type: "pick", to, authored, user: "u_fake", address: undefined }));
const previewAnswers = [accepted, { outcome: "unavailable", message: "no engine" } as const, rejected].map((output) => ({ type: "xstate.done.actor.preview", output }) as unknown as Event);
const walkEvents: Event[] = [
  { type: "start", from: "n_a" },
  { type: "start", from: "n_b" },
  ...picks,
  ...previewAnswers,
  { type: "xstate.error.actor.preview", error: new Error("the engine failed") } as unknown as Event,
  { type: "write.landed" },
  { type: "write.unlanded" },
  { type: "write.busy" },
  { type: "write.ended" },
  { type: "write.missing" },
  { type: "cancel" },
];

function walk(compatible: boolean): Walk {
  const options = {
    input: walkInput(compatible),
    events: walkEvents,
    // The context holds refs and functions: serialize what the machine decides on.
    serializeState: (snapshot: DrawingSnapshot) => {
      const { from, refused, picked, sent } = snapshot.context;
      return JSON.stringify([snapshot.value, from, refused?.by ?? null, picked?.to ?? null, sent?.to ?? null]);
    },
  };
  const map = getAdjacencyMap(edgeDrawing, options) as Record<string, { state: DrawingSnapshot; transitions: Record<string, Step> }>;
  const paths = getShortestPaths(edgeDrawing, options) as { state: DrawingSnapshot }[];
  return { adjacency: Object.values(map).map(({ state, transitions }) => ({ state, steps: Object.values(transitions) })), reached: paths.map(({ state }) => state) };
}

const walks = { free: walk(true), skewed: walk(false) };
const inMode = (snapshot: DrawingSnapshot, mode: object | string) => snapshot.matches(mode as never);
const SUBMITTING = { active: "submitting" };
const AWAITING = { active: { submitting: "awaiting" } };
const UNLANDED = { active: { choosing: "unlanded" } };
const MODES = ["inactive", { active: { choosing: "fresh" } }, UNLANDED, { active: { submitting: "previewing" } }, AWAITING];

describe("every state the drawing can reach (A3, H5)", () => {
  it("holds in every state only what its mode needs", () => {
    for (const { state } of [...walks.free.adjacency, ...walks.skewed.adjacency]) {
      const { from, picked, sent, refused } = state.context;
      if (inMode(state, "inactive")) {
        expect([from, picked, sent, refused]).toEqual([undefined, undefined, undefined, undefined]);
        continue;
      }
      expect(from).toBeDefined();
      if (inMode(state, { active: { submitting: "previewing" } })) {
        expect([picked !== undefined, sent]).toEqual([true, undefined]);
      } else if (inMode(state, AWAITING) || inMode(state, UNLANDED)) {
        expect([picked, sent !== undefined]).toEqual([undefined, true]);
      } else {
        expect([picked, sent]).toEqual([undefined, undefined]);
      }
    }
  });

  it("reaches every mode, and a pick in skew goes straight to the write", () => {
    const reaches = (plan: Walk, mode: object | string) => plan.reached.some((state) => inMode(state, mode));
    expect(MODES.map((mode) => reaches(walks.free, mode))).toEqual(MODES.map(() => true));
    expect(reaches(walks.skewed, { active: { submitting: "previewing" } })).toBe(false);
    expect(reaches(walks.skewed, AWAITING)).toBe(true);
  });
});

describe("every way out of the drawing is one reset (A3)", () => {
  it("leads to the initial state from stopping an active drawing and from its write landing, wherever it is", () => {
    const [initial] = initialTransition(edgeDrawing, walkInput(true));
    for (const plan of [walks.free, walks.skewed]) {
      for (const { state, steps } of plan.adjacency) {
        const stops = steps.filter(({ event }) => event.type === "cancel");
        expect(stops.length > 0 || inMode(state, "inactive")).toBe(true);
        const lands = steps.filter(({ event }) => event.type === "write.landed" && (inMode(state, AWAITING) || inMode(state, UNLANDED)));
        for (const { state: after } of [...stops, ...lands]) {
          expect(after.value).toEqual(initial.value);
          expect(after.context).toMatchObject({ from: undefined, refused: undefined, picked: undefined, sent: undefined });
        }
      }
    }
  });
});

describe("a busy drawing hears no pick or start (H5)", () => {
  it("stays where it is, with the same pick, write and node", () => {
    const busy = walks.free.adjacency.filter(({ state }) => inMode(state, SUBMITTING));
    expect(busy.length).toBeGreaterThan(0);
    for (const { state, steps } of busy) {
      for (const { event, state: after } of steps.filter(({ event: heard }) => heard.type === "pick" || heard.type === "start")) {
        expect([event.type, after.value, after.context.picked?.to, after.context.sent?.to, after.context.from]).toEqual([event.type, state.value, state.context.picked?.to, state.context.sent?.to, state.context.from]);
      }
    }
  });
});
