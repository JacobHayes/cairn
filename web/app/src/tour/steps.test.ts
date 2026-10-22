// The steps' gates and copy over the practice journey as the real engine derives it: each gate
// is a predicate on derived state, and each sentence names what is true now.
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import type { InBrowserHost } from "@cairn/wasm";

import { derivedJourney, journeyAfter, seeded } from "../authoring/engine.test-support.ts";
import { transition } from "../detail/model.ts";
import { PRACTICE, practiceMutations } from "./practice.ts";
import { clampCursor, exampleReply, factsOf, stepDone, stepsFor, type Facts, type StepId, type Where } from "./steps.ts";

const { decision, draft, comments, review, summary } = PRACTICE;

let host: InBrowserHost;
beforeAll(async () => {
  host = await seeded();
});
const freed: (() => void)[] = [];
afterAll(() => {
  freed.forEach((free) => {
    free();
  });
});

type Stage = "created" | "yes" | "no" | "yes-draft-done" | "no-draft-done";

/** The facts of a practice journey after `stage`. */
function factsAt(stage: Stage): Facts {
  const id = `j_steps_${stage.replaceAll("-", "_")}`;
  host.patch({ patch: { id: `p_${id}`, target: { journey: id }, base_revision: 0, mutations: practiceMutations("2026-10-06") } });
  let text = host.documentText(id);
  const said = stage.startsWith("yes") ? true : stage.startsWith("no") ? false : undefined;
  if (said !== undefined) {
    text = journeyAfter(host, id, [{ op: "answer", decision, value: { boolean: said } }], text);
  }
  if (stage.endsWith("draft-done")) {
    text = journeyAfter(host, id, [transition(draft, "complete")], text);
  }
  const derived = derivedJourney(host, text);
  freed.push(derived.free);
  return factsOf(derived.ready);
}

const none: Where = { selected: undefined, frontier: false, replied: false };
const step = (id: StepId) => {
  const found = stepsFor(true).find((each) => each.id === id);
  if (found === undefined) {
    throw new Error(`no step ${id}`);
  }
  return found;
};

describe("the gates", () => {
  it("the map waits for the decision to be selected", () => {
    const facts = factsAt("created");
    expect(stepDone(step("map"), facts, none)).toBe(false);
    expect(stepDone(step("map"), facts, { ...none, selected: comments })).toBe(false);
    expect(stepDone(step("map"), facts, { ...none, selected: decision })).toBe(true);
  });

  it("the answer step waits for an answer in effect, either one", () => {
    expect(stepDone(step("answer"), factsAt("created"), none)).toBe(false);
    expect(stepDone(step("answer"), factsAt("yes"), none)).toBe(true);
    expect(stepDone(step("answer"), factsAt("no"), none)).toBe(true);
  });

  it("Frontier waits for the highlight, and finishing waits for the draft to be done", () => {
    const facts = factsAt("yes");
    expect(stepDone(step("frontier"), facts, none)).toBe(false);
    expect(stepDone(step("frontier"), facts, { ...none, frontier: true })).toBe(true);
    expect(stepDone(step("finish"), facts, none)).toBe(false);
    expect(stepDone(step("finish"), factsAt("yes-draft-done"), none)).toBe(true);
  });

  it("the dates have no action, and the assistant step waits for the reply", () => {
    expect(stepDone(step("dates"), factsAt("yes"), none)).toBe(true);
    expect(stepDone(step("assistant"), factsAt("yes"), none)).toBe(false);
    expect(stepDone(step("assistant"), factsAt("yes"), { ...none, replied: true })).toBe(true);
  });
});

describe("the copy names what is true now", () => {
  const titles = (facts: Facts, keys: string[]) => keys.map((key) => facts.title(key));

  it("the map says the review waits for a decision until it is answered", () => {
    const text = step("map").instruction(factsAt("created"), false);
    expect(text).toContain(factsAt("created").title(review));
    expect(text).toContain("waits for a decision");
    expect(step("map").instruction(factsAt("no"), false)).not.toContain("waits for a decision");
  });

  it("the answer's result says whether the review applies", () => {
    const [yes, no] = [factsAt("yes"), factsAt("no")];
    expect(step("answer").result?.(yes)).toContain("applies now");
    expect(step("answer").result?.(no)).toContain("does not apply");
  });

  it("Frontier names what can move, which is the draft once the decision is answered", () => {
    const facts = factsAt("yes");
    expect(facts.frontier).toEqual([draft]);
    expect(step("frontier").result?.(facts)).toContain(facts.title(draft));
  });

  it("finishing names one freed item after a No and two after a Yes, in the singular and the plural", () => {
    const [yes, no] = [factsAt("yes-draft-done"), factsAt("no-draft-done")];
    const named = (facts: Facts) => titles(facts, [comments, review, summary]).filter((title) => (step("finish").result?.(facts) ?? "").includes(title));
    expect(named(no)).toEqual([no.title(comments)]);
    expect(named(yes)).toEqual([yes.title(comments), yes.title(review)]);
    expect(step("finish").result?.(no)).toMatch(/ is ready now/);
    expect(step("finish").result?.(yes)).toMatch(/ are ready now/);
  });

  it("the example reply names the same freed items, and what still waits for them", () => {
    const [yes, no] = [factsAt("yes-draft-done"), factsAt("no-draft-done")];
    expect(exampleReply(yes)).toContain(yes.title(review));
    expect(exampleReply(no)).not.toContain(no.title(review));
    expect(exampleReply(no)).toContain(no.title(summary));
  });

  it("the phone's instruction sends the person to Controls", () => {
    const facts = factsAt("yes");
    expect(step("frontier").instruction(facts, true)).toContain("Controls");
    expect(step("frontier").instruction(facts, false)).not.toContain("Controls");
  });
});

describe("the list of steps", () => {
  it("has no assistant step where none is offered, and the count shown is the list's length", () => {
    const without = stepsFor(false);
    const withIt = stepsFor(true);
    expect(without.map((each) => each.id)).not.toContain("assistant");
    expect(withIt.length).toBe(without.length + 1);
    expect(withIt.at(-1)?.id).toBe("assistant");
  });

  it("clamps a cursor into the list", () => {
    const list = stepsFor(false);
    expect(clampCursor(undefined, list)).toBe(0);
    expect(clampCursor(-3, list)).toBe(0);
    expect(clampCursor(99, list)).toBe(list.length - 1);
    expect(clampCursor(2, list)).toBe(2);
  });
});
