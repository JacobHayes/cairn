// The practice journey against the real engine: the states the tour's words rely on, after each
// move the tour asks of a person. The copy is composed from derived state, so this is the
// contract it stands on.
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import type { InBrowserHost } from "@cairn/wasm";

import { derivedJourney, journeyAfter, seeded } from "../authoring/engine.test-support.ts";
import { transition } from "../detail/model.ts";
import { addDays, practiceMutations, PRACTICE } from "./practice.ts";

const TODAY = "2026-10-06";

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

/** The practice journey `id`, created at base revision 0 as the tour sends it. */
function created(id: string): string {
  host.patch({ patch: { id: `p_create_${id}`, target: { journey: id }, base_revision: 0, mutations: practiceMutations(TODAY) } });
  return host.documentText(id);
}

/** The states derived from `text`, by display state. */
function states(text: string): Record<string, string> {
  const derived = derivedJourney(host, text);
  freed.push(derived.free);
  return Object.fromEntries(Object.entries(derived.ready.derived.nodes).map(([key, node]) => [key, node.display_state]));
}

const keysIn = (states: Record<string, string>, state: string): string[] =>
  Object.entries(states)
    .filter(([, each]) => each === state)
    .map(([key]) => key)
    .sort();

const { decision, draft, comments, review, summary, meeting } = PRACTICE;
const answer = (value: boolean) => ({ op: "answer" as const, decision, value: { boolean: value } });

describe("the practice journey", () => {
  it("starts with the decision and the draft ready and the review waiting on the decision", () => {
    const text = created("j_practice_created");
    const now = states(text);
    expect(keysIn(now, "ready")).toEqual([decision, draft].sort());
    expect(keysIn(now, "blocked")).toEqual([comments, meeting, summary].sort());
    expect(keysIn(now, "conditional")).toEqual([review]);
  });

  it.each([
    { said: true, ready: [comments, review], applies: true },
    { said: false, ready: [comments], applies: false },
  ])("after the decision is answered $said and the draft is done, $ready are ready", ({ said, ready, applies }) => {
    const id = `j_practice_answered_${String(said)}`;
    const answered = journeyAfter(host, id, [answer(said)], created(id));
    const decided = states(answered);
    expect(keysIn(decided, "ready")).toEqual([draft]);
    expect(decided[review]).toBe(applies ? "blocked" : "not_relevant");
    const finished = journeyAfter(host, id, [transition(draft, "complete")], answered);
    const after = states(finished);
    expect(keysIn(after, "ready")).toEqual([...ready].sort());
    expect(keysIn(after, "blocked")).toEqual([meeting, summary].sort());
  });

  it("pins the meeting fourteen days after the given today and gives the summary the day before", () => {
    const text = created("j_practice_dates");
    const derived = derivedJourney(host, text);
    freed.push(derived.free);
    expect(derived.ready.derived.nodes[meeting]?.dates.effective_date).toEqual({ date: addDays(TODAY, 14), origin: "pin" });
    expect(derived.ready.derived.nodes[summary]?.dates.due?.date).toBe(addDays(TODAY, 13));
  });
});

describe("addDays", () => {
  it("crosses month and year ends and leap days", () => {
    expect(addDays("2026-12-25", 14)).toBe("2027-01-08");
    expect(addDays("2028-02-20", 10)).toBe("2028-03-01");
    expect(addDays("2026-03-01", -1)).toBe("2026-02-28");
  });
});
