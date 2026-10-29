// The steps' gates and copy over the practice journey as the real engine derives it: each gate
// is a predicate on derived state, and each sentence names what is true now.
import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { derivedJourney, journeyAfter, seeded, type Fixtures } from "../authoring/engine.test-support.ts";
import { transition } from "../detail/model.ts";
import { PRACTICE, practiceMutations } from "./practice.ts";
import { cardOf, clampCursor, exampleReply, factsOf, rowOf, stepDone, stepsFor, type Facts, type StepId, type Where, type WindowKind } from "./steps.ts";

const { decision, draft, comments, review, summary } = PRACTICE;
const wide: WindowKind = { phone: false, short: false };

let host: Fixtures;
const facts = new Map<Stage, Facts>();
beforeAll(async () => {
  host = await seeded();
  for (const stage of STAGES) {
    facts.set(stage, await factsAfter(stage));
  }
});
const freed: (() => void)[] = [];
afterAll(() => {
  freed.forEach((free) => {
    free();
  });
});

const STAGES = ["created", "yes", "no", "yes-draft-done", "no-draft-done"] as const;
type Stage = (typeof STAGES)[number];

/** The facts of a practice journey after `stage`. */
function factsAt(stage: Stage): Facts {
  const found = facts.get(stage);
  if (found === undefined) {
    throw new Error(`no facts for ${stage}`);
  }
  return found;
}

/** Makes a practice journey as the tour does and takes it to `stage`, derived. */
async function factsAfter(stage: Stage): Promise<Facts> {
  const id = `j_steps_${stage.replaceAll("-", "_")}`;
  await host.patch({ patch: { id: `p_${id}`, target: { journey: id }, base_revision: 0, mutations: practiceMutations("2026-10-06") } });
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
    const text = step("map").instruction(factsAt("created"), wide);
    expect(text).toContain(factsAt("created").title(review));
    expect(text).toContain("waits for a decision");
    expect(step("map").instruction(factsAt("no"), wide)).not.toContain("waits for a decision");
  });

  it("the map in a short window leaves out the cards that window cannot show beside the decision", () => {
    const text = step("map").instruction(factsAt("created"), { phone: false, short: true });
    expect(text).toContain(factsAt("created").title(decision));
    expect(text).not.toContain(factsAt("created").title(review));
    expect(text).not.toContain(factsAt("created").title(draft));
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
    expect(step("frontier").instruction(facts, { phone: true, short: false })).toContain("Controls");
    expect(step("frontier").instruction(facts, wide)).not.toContain("Controls");
  });
});

describe("what a step points at once its action is done", () => {
  const anchored = (id: StepId, facts: Facts) => step(id).anchor?.(facts, none);

  it("the answer's change is the review card after a Yes and the summary card after a No, where the review has left the graph", () => {
    expect(anchored("answer", factsAt("yes"))).toEqual([cardOf(review)]);
    expect(anchored("answer", factsAt("no"))).toEqual([cardOf(summary)]);
  });

  it("Frontier's change is the first card that can move, whichever it is", () => {
    const facts = factsAt("yes");
    expect(anchored("frontier", facts)).toEqual([cardOf(facts.frontier[0] ?? "")]);
    expect(anchored("frontier", { ...facts, frontier: [comments, draft] })).toEqual([cardOf(comments)]);
    expect(anchored("frontier", { ...facts, frontier: [] })).toEqual([]);
  });

  it("finishing's change is the rows it freed: the comments alone after a No, the review too after a Yes", () => {
    expect(anchored("finish", factsAt("no-draft-done"))).toEqual([rowOf(comments)]);
    expect(anchored("finish", factsAt("yes-draft-done"))).toEqual([rowOf(comments), rowOf(review)]);
    expect(anchored("finish", factsAt("yes"))).toEqual([]);
  });

  it("a step with no action reveals nothing, and every other step has somewhere to point once done", () => {
    for (const each of stepsFor(true)) {
      expect(each.reveal === "none").toBe(each.done === undefined);
      if (each.reveal !== "none") {
        expect(anchored(each.id, factsAt("yes-draft-done")), each.id).not.toEqual([]);
      }
    }
  });

  it("the answer's result says the review is ready when the draft is already done (Back after finishing)", () => {
    expect(step("answer").result?.(factsAt("yes-draft-done"))).toContain("is ready");
    expect(step("answer").result?.(factsAt("yes"))).toContain("waits for the draft");
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
