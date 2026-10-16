// C19, B13: inserting a segment, the stepper's three steps as a machine: choose a segment,
// place it, and, when the segment declares roles or kinds, say who they become here. The step is
// the machine's state, so no move can leave it somewhere the steps do not go:
//
//   choosing -- choose --> placing -- next (when there are people to map) --> mapping
//   choosing <-- back --- placing <-- back, or planned (no people, after all) --- mapping
//
// What the person has chosen (the segment and version, the choices, and the key the insertion
// is minted under) is the machine's context and is kept as a plain draft whenever it changes,
// so a reload picks the stepper up where it was. A draft restored from storage is corrected on
// the way in (`heldOf`): whatever is not a valid draft is a fresh one, a step past the first
// without a segment is the first, and nodes the graph no longer holds are dropped from it. The
// insertion key is minted once, when no valid draft names one, and never again (B13: the
// preview and the apply write the same keys).
import { assign, setup, type SnapshotFrom } from "xstate";

import { mintKey } from "../authoring/keys.ts";
import type { Tree } from "../authoring/graph.ts";
import { NO_CHOICES, type Choices, type Mapping } from "./model.ts";

export type Step = 1 | 2 | 3;

/** The segment and version chosen. */
export interface Picked {
  id: string;
  version: number;
}

/** What the stepper keeps of an insertion not yet reviewed: the step, the segment and version, the choices, and the key it will mint. */
export interface Held {
  step: Step;
  picked: Picked | undefined;
  choices: Choices;
  insertion: string;
}

const isPlain = (value: unknown): value is Record<string, unknown> => typeof value === "object" && value !== null && !Array.isArray(value);
const isText = (value: unknown): value is string => typeof value === "string";

function mappingsOf(value: unknown): Record<string, Mapping> {
  const found: Record<string, Mapping> = {};
  if (isPlain(value)) {
    for (const [key, mapping] of Object.entries(value)) {
      if (mapping === "add") {
        found[key] = "add";
      } else if (isPlain(mapping) && isText(mapping.existing)) {
        found[key] = { existing: mapping.existing };
      }
    }
  }
  return found;
}

/** The nodes of `value` the graph holds, in order. */
function heldNodes(value: unknown, tree: Tree): string[] {
  return Array.isArray(value) ? value.filter((key): key is string => isText(key) && tree.byKey.has(key)) : [];
}

/**
 * The stepper's draft as it opens: `kept` (whatever session storage held under the draft's key)
 * corrected against `tree`, or a fresh draft at the first step under `parent` when it is not
 * one. `mint` makes the insertion key when none valid is kept.
 */
export function heldOf(kept: unknown, tree: Tree, parent: string | undefined, mint: () => string = () => mintKey("i_")): Held {
  if (!isPlain(kept) || !isPlain(kept.choices)) {
    return { step: 1, picked: undefined, choices: { ...NO_CHOICES, parent }, insertion: mint() };
  }
  const { choices } = kept;
  const picked = isPlain(kept.picked) && isText(kept.picked.id) && Number.isInteger(kept.picked.version) ? { id: kept.picked.id, version: kept.picked.version as number } : undefined;
  const asked = isText(choices.parent) && tree.byKey.has(choices.parent) ? choices.parent : undefined;
  const step = picked === undefined ? 1 : kept.step === 3 ? 3 : kept.step === 2 ? 2 : 1;
  return {
    step,
    picked,
    choices: {
      parent: asked,
      title: isText(choices.title) ? choices.title : "",
      after: heldNodes(choices.after, tree),
      before: heldNodes(choices.before, tree),
      roles: mappingsOf(choices.roles),
      kinds: mappingsOf(choices.kinds),
    },
    insertion: isText(kept.insertion) && kept.insertion.startsWith("i_") ? kept.insertion : mint(),
  };
}

/** What the stepper is started with: the draft it opens at, and where to keep it. */
export interface InsertionInput {
  held: Held;
  keep: (held: Held) => void;
}

interface InsertionContext {
  /** The step the stepper opened at; the state is the step from then on. */
  opened: Step;
  picked: Picked | undefined;
  choices: Choices;
  insertion: string;
  keep: (held: Held) => void;
}

export type InsertionEvent =
  | { type: "choose"; id: string; version: number }
  | { type: "change"; next: Partial<Choices> }
  /** To the people step; `people`: the plan has people to map. */
  | { type: "next"; people: boolean }
  | { type: "back" }
  /** The plan is known: `people` says whether it has people to map. */
  | { type: "planned"; people: boolean };

export const insertion = setup({
  types: { context: {} as InsertionContext, events: {} as InsertionEvent, input: {} as InsertionInput },
  guards: {
    opensAt: ({ context }, step: Step) => context.opened === step,
    people: ({ event }) => (event.type === "next" || event.type === "planned") && event.people,
    noPeople: ({ event }) => event.type === "planned" && !event.people,
  },
  actions: {
    // Keeps what is chosen as a plain draft: the step is the state being entered.
    keep: ({ context }, step: Step) => {
      context.keep({ step, picked: context.picked, choices: context.choices, insertion: context.insertion });
    },
    chosen: assign(({ context, event }) =>
      event.type === "choose" ? { picked: { id: event.id, version: event.version }, choices: { ...NO_CHOICES, parent: context.choices.parent } } : {},
    ),
    changed: assign(({ context, event }) => (event.type === "change" ? { choices: { ...context.choices, ...event.next } } : {})),
  },
}).createMachine({
  id: "insertion",
  context: ({ input }) => ({
    opened: input.held.step,
    picked: input.held.picked,
    choices: input.held.choices,
    insertion: input.held.insertion,
    keep: input.keep,
  }),
  initial: "start",
  states: {
    start: {
      always: [
        { guard: { type: "opensAt", params: 3 }, target: "mapping" },
        { guard: { type: "opensAt", params: 2 }, target: "placing" },
        { target: "choosing" },
      ],
    },
    choosing: {
      entry: { type: "keep", params: 1 },
      on: { choose: { target: "placing", actions: "chosen" } },
    },
    placing: {
      entry: { type: "keep", params: 2 },
      on: {
        change: { actions: ["changed", { type: "keep", params: 2 }] },
        next: { guard: "people", target: "mapping" },
        back: "choosing",
      },
    },
    mapping: {
      entry: { type: "keep", params: 3 },
      on: {
        change: { actions: ["changed", { type: "keep", params: 3 }] },
        back: "placing",
        // A restored people step with nobody to map falls back, once the plan is known to be that.
        planned: { guard: "noPeople", target: "placing" },
      },
    },
  },
});

export type InsertionSnapshot = SnapshotFrom<typeof insertion>;

/** The step the stepper is at. */
export function stepOf(snapshot: InsertionSnapshot): Step {
  return snapshot.matches("choosing") ? 1 : snapshot.matches("placing") ? 2 : 3;
}
