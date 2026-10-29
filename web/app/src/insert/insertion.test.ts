// C19, B13: the stepper's steps are a machine whose state is the step, a draft restored from
// storage is corrected on the way in, and the key the insertion is minted under is made once.
import { createActor } from "xstate";
import { describe, expect, it } from "vitest";

import { treeOf, type GraphNode } from "../authoring/graph.ts";
import { heldOf, insertion, stepOf, type Held, type InsertionEvent, type Step } from "./insertion.ts";
import { NO_CHOICES } from "./model.ts";

const node = (key: string): GraphNode => ({ key, id: key.replace("n_", ""), kind: "decision", title: key });
const tree = treeOf({ nodes: [node("n_a"), node("n_b"), node("n_phase")] });

const held = (step: Step, extra: Partial<Held> = {}): Held => ({
  step,
  picked: step === 1 ? undefined : { id: "security-review", version: 1 },
  choices: { ...NO_CHOICES, parent: undefined },
  insertion: "i_kept",
  ...extra,
});

function started(at: Held) {
  const kept: Held[] = [];
  const actor = createActor(insertion, {
    input: {
      held: at,
      keep: (next) => {
        kept.push(next);
      },
    },
  });
  actor.start();
  return { actor, kept };
}

/** The state each event leaves the machine in from each step; an event with no entry leaves it where it was. */
const events: Record<string, InsertionEvent> = {
  choose: { type: "choose", id: "other", version: 2 },
  change: { type: "change", next: { title: "Typed" } },
  "next with people": { type: "next", people: true },
  "next without people": { type: "next", people: false },
  back: { type: "back" },
  "planned with people": { type: "planned", people: true },
  "planned without people": { type: "planned", people: false },
};
const moves: Record<Step, Record<string, Step>> = {
  1: { choose: 2 },
  2: { "next with people": 3, back: 1 },
  3: { back: 2, "planned without people": 2 },
};

describe("the steps", () => {
  it.each([1, 2, 3] as Step[])("from step %i, every event goes where the steps go, and nowhere else", (from) => {
    for (const [name, event] of Object.entries(events)) {
      const { actor } = started(held(from));
      actor.send(event);
      expect([name, stepOf(actor.getSnapshot())]).toEqual([name, moves[from][name] ?? from]);
    }
  });

  it.each([1, 2, 3] as Step[])("opens at the step it was restored at (%i)", (step) => {
    expect(stepOf(started(held(step)).actor.getSnapshot())).toBe(step);
  });

  it("starts the choices afresh, under the parent already chosen, when another route is chosen", () => {
    const { actor, kept } = started(held(1, { choices: { ...NO_CHOICES, parent: "n_phase", title: "Old" } }));
    actor.send({ type: "choose", id: "other", version: 2 });
    expect(kept.at(-1)).toEqual({ step: 2, picked: { id: "other", version: 2 }, choices: { ...NO_CHOICES, parent: "n_phase" }, insertion: "i_kept" });
  });
});

describe("what is kept", () => {
  it("is the entered step with everything chosen, kept again on each change and on each move", () => {
    const { actor, kept } = started(held(2));
    actor.send({ type: "change", next: { title: "Typed" } });
    actor.send({ type: "next", people: true });
    actor.send({ type: "change", next: { after: ["n_a"] } });
    actor.send({ type: "back" });
    expect(kept.map((each) => each.step)).toEqual([2, 2, 3, 3, 2]);
    expect(kept.at(-1)?.choices).toMatchObject({ title: "Typed", after: ["n_a"] });
  });

  it("keeps the insertion key the stepper opened with through every step, however it moves", () => {
    const { actor, kept } = started(held(1));
    actor.send({ type: "choose", id: "other", version: 2 });
    actor.send({ type: "next", people: true });
    actor.send({ type: "back" });
    actor.send({ type: "back" });
    expect(new Set(kept.map((each) => each.insertion))).toEqual(new Set(["i_kept"]));
  });

  it("keeps a restored people step until the plan is known not to have people", () => {
    const { actor, kept } = started(held(3));
    actor.send({ type: "planned", people: true });
    expect(stepOf(actor.getSnapshot())).toBe(3);
    actor.send({ type: "planned", people: false });
    expect(stepOf(actor.getSnapshot())).toBe(2);
    expect(kept.at(-1)?.step).toBe(2);
  });
});

describe("heldOf", () => {
  const mint = () => {
    let minted = 0;
    return { mint: () => `i_minted${String(++minted)}`, count: () => minted };
  };

  it.each([undefined, null, "text", 3, [], {}, { choices: [] }, { choices: null }])("is a fresh draft at the first step under the parent, with one key minted, for %j", (kept) => {
    const made = mint();
    expect(heldOf(kept, tree, "n_phase", made.mint)).toEqual({ step: 1, picked: undefined, choices: { ...NO_CHOICES, parent: "n_phase" }, insertion: "i_minted1" });
    expect(made.count()).toBe(1);
  });

  it("mints nothing when the draft names a valid key, and keeps the draft whole", () => {
    const made = mint();
    const draft = {
      step: 3,
      picked: { id: "security-review", version: 2 },
      choices: { parent: "n_phase", title: "Review", after: ["n_a"], before: ["n_b"], roles: { r_x: "add", r_y: { existing: "r_z" } }, kinds: {} },
      insertion: "i_kept",
    };
    expect(heldOf(draft, tree, undefined, made.mint)).toEqual(draft);
    expect(made.count()).toBe(0);
  });

  it("puts a step past the first back to the first when no route is chosen", () => {
    const draft = { step: 3, picked: undefined, choices: { ...NO_CHOICES }, insertion: "i_kept" };
    expect(heldOf(draft, tree, undefined).step).toBe(1);
  });

  it("drops the nodes the graph no longer holds, and a parent that is gone", () => {
    const draft = {
      step: 2,
      picked: { id: "s", version: 1 },
      choices: { parent: "n_gone", title: "T", after: ["n_a", "n_gone", 4], before: ["n_gone"], roles: {}, kinds: {} },
      insertion: "i_kept",
    };
    expect(heldOf(draft, tree, "n_phase").choices).toMatchObject({ parent: undefined, after: ["n_a"], before: [] });
  });

  it("drops a role or kind mapping that is neither added nor an existing one, and mints a new key for a bad key", () => {
    const made = mint();
    const draft = {
      step: 3,
      picked: { id: "s", version: 1 },
      choices: { parent: undefined, title: "", after: [], before: [], roles: { r_a: "add", r_b: "nonsense", r_c: { existing: 4 } }, kinds: { k_a: { existing: "k_b" } } },
      insertion: "not-a-key",
    };
    const corrected = heldOf(draft, tree, undefined, made.mint);
    expect(corrected.choices.roles).toEqual({ r_a: "add" });
    expect(corrected.choices.kinds).toEqual({ k_a: { existing: "k_b" } });
    expect(corrected.insertion).toBe("i_minted1");
  });

  it("drops a picked route whose version is not a whole number", () => {
    const draft = { step: 2, picked: { id: "s", version: 1.5 }, choices: { ...NO_CHOICES }, insertion: "i_kept" };
    expect(heldOf(draft, tree, undefined)).toMatchObject({ picked: undefined, step: 1 });
  });
});
