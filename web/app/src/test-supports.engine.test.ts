// The hand-written derived journeys the unit tests of screens share (canvas.test-support.ts,
// view.test-support.ts) against the real engine: the engine derives each one's graph and
// state again, and every node's display state, relevance, whether it can be acted on, what
// blocks it and which guards fail must be what the engine says. A state the engine cannot
// produce fails here, so a screen is never tested against an answer the derive could not give.
import type { Schema } from "@cairn/client";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { derivedJourney, seeded, type Fixtures } from "./authoring/engine.test-support.ts";
import { canvasView } from "./canvas/canvas.test-support.ts";
import type { Ready } from "./detail/model.ts";
import { testView } from "./detail/view.test-support.ts";

let host: Fixtures;
beforeAll(async () => {
  host = await seeded();
});
const freed: (() => void)[] = [];
afterAll(() => { freed.forEach((free) => { free(); }); });

/** What a node's derive says that a screen reads and the engine decides alone. */
function facts(derived: Ready["derived"]["nodes"][string]) {
  return {
    display_state: derived.display_state,
    relevance: derived.relevance.value,
    actionable: derived.actionable,
    snoozable: derived.snoozable === true,
    open: derived.open === true,
    moves: [...(derived.moves ?? [])].sort(),
    snooze_lifts: derived.snooze_lifts ?? null,
    skips_beneath: derived.skips_beneath === true,
    skip_count: derived.skip_count ?? 0,
    answer: derived.answer ?? null,
    blocked_by: [...(derived.blocked_by ?? [])].map((each) => each.node).sort(),
    failing_guards: [...(derived.failing_guards ?? [])].sort(),
  };
}

const INITIAL: Record<string, string> = { decision: "open", deliverable: "todo", action: "todo", milestone: "pending", group: "derived" };

/** `journey` with the stored state the engine requires of every node: the kind's initial one where the support names none. */
function withStoredStates(journey: Ready["journey"]) {
  const state = structuredClone(journey.graph.state ?? {}) as { nodes?: Record<string, unknown>; answers?: Record<string, unknown> };
  state.nodes ??= {};
  for (const node of journey.graph.nodes ?? []) {
    const decided = node.kind === "decision" && state.answers?.[node.key] !== undefined;
    state.nodes[node.key] ??= { state: decided ? "decided" : INITIAL[node.kind], provenance: "from_route" };
  }
  return { ...journey, graph: { ...journey.graph, state } };
}

describe.each([
  ["the canvas journey", canvasView],
  ["the node detail journey", testView],
])("%s", (_name, build) => {
  it("is what the engine derives from its graph and state", () => {
    const view = build();
    const document = { journey: withStoredStates(view.journey), inputs: view.inputs } as unknown as Schema<"DomainDocument">;
    let derived: ReturnType<typeof derivedJourney>;
    try { derived = derivedJourney(host, JSON.stringify(document)); } catch (e) { console.log(JSON.stringify((e as { reason?: unknown }).reason)); throw e; }
    freed.push(derived.free);
    const mine = Object.fromEntries(Object.entries(view.derived.nodes).map(([key, each]) => [key, facts(each)]));
    const theirs = Object.fromEntries(Object.keys(view.derived.nodes).map((key) => [key, facts(derived.ready.derived.nodes[key] as Ready["derived"]["nodes"][string])]));
    expect(mine).toEqual(theirs);
    expect({ stages: view.derived.current_stages ?? [], final: view.derived.final_milestone ?? null }).toEqual({ stages: derived.ready.derived.current_stages ?? [], final: derived.ready.derived.final_milestone ?? null });
  });
});
