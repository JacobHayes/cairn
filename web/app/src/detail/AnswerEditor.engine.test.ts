// D4, D8 against the real engine: answering a decision anyway sends a bypass the engine accepts,
// whichever guards hold it, its own date among them.
import type { InBrowserHost } from "@cairn/wasm";
import { afterEach, beforeAll, describe, expect, it } from "vitest";

import { derivedJourney, journeyAfter, seeded, type DerivedJourney } from "../authoring/engine.test-support.ts";
import type { Mutation } from "../authoring/graph.ts";
import { answerBypass } from "./AnswerEditor.tsx";

let host: InBrowserHost;
const held: DerivedJourney[] = [];

beforeAll(async () => {
  host = await seeded();
});

afterEach(() => {
  held.splice(0).forEach((each) => { each.free(); });
});

const answer: Mutation = { op: "answer", decision: "n_debrief", value: { text: "Written" } };

describe("D4, D8: answering anyway", () => {
  it.each([
    ["its own date, still ahead", [{ op: "add_node", node: { key: "n_debrief", id: "debrief", kind: "decision", title: "Debrief format", prompt: "Which?", answer_type: "text", not_before: { after: "n_launch" } } }]],
    [
      "its own date and what it waits on",
      [
        { op: "add_node", node: { key: "n_debrief", id: "debrief", kind: "decision", title: "Debrief format", prompt: "Which?", answer_type: "text", not_before: { after: "n_launch" } } },
        { op: "add_edge", edge: { node: "n_debrief", requires: "n_beta" } },
      ],
    ],
  ] satisfies [string, Mutation[]][])("bypasses %s, which a plain answer is refused for", (_, added) => {
    const text = journeyAfter(host, "j_launch", added);
    const journey = derivedJourney(host, text);
    held.push(journey);
    const derived = journey.ready.derived.nodes.n_debrief;
    if (derived === undefined) {
      throw new Error("no n_debrief");
    }
    expect(derived.failing_guards).toContain("date_arrived");
    expect(() => journeyAfter(host, "j_launch", [answer], text)).toThrow();
    const bypassed = journeyAfter(host, "j_launch", [...answerBypass("n_debrief", derived, "The team asked early"), answer], text);
    expect(bypassed).toContain("Written");
    // The reason is optional: an anyway form opened and left empty bypasses all the same.
    const reasonless = answerBypass("n_debrief", derived, "");
    expect(reasonless).toHaveLength(1);
    expect(journeyAfter(host, "j_launch", [...reasonless, answer], text)).toContain("Written");
    expect(answerBypass("n_debrief", derived, undefined)).toEqual([]);
  });
});
