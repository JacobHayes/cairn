// The assistant panel's model (I5): the conversation as kept, with the turns this tab sent
// shown as their writes, and the tab's kept turns bounded.
import { describe, expect, it } from "vitest";

import type { AssistantAction, ConversationEntry, Ended } from "../data/assistant.ts";
import { KEPT_TURN_COUNT_MAX, answeredEntries, holdsTurn, keeping, latestAnswerAt, transcript, type KeptTurn } from "./model.ts";

const said = (at: string, content: string): ConversationEntry => ({ author: "user", at, content });
const report = (at: string, content: string): ConversationEntry => ({ author: "cairn", at, content });
const reply = (at: string, content: string): ConversationEntry => ({ author: "assistant", at, content });

const applied: AssistantAction = {
  outcome: "applied",
  tool: "transition_node",
  receipt: { patch_id: "p_one", domain: { journey: "j_one" }, content_hash: "h", revision: 3 },
  nodes: ["n_kickoff"],
};
const proposed: AssistantAction = { outcome: "proposed", tool: "apply_patch", proposal: "pr_one", because: { reason: "structural" } };
const replied: Ended = { status: "replied" };

const messages = [
  said("2026-10-07T10:00:00Z", "Kickoff happened; add a step."),
  report("2026-10-07T10:00:05Z", "Applied."),
  report("2026-10-07T10:00:05Z", "Drafted."),
  reply("2026-10-07T10:00:05Z", "Done."),
];

describe("transcript (I5)", () => {
  it("shows a turn this tab sent as its writes, in order, between the words and the reply", () => {
    const kept: KeptTurn[] = [{ at: "2026-10-07T10:00:05Z", actions: [applied, proposed], ended: replied }];
    expect(transcript(messages, kept).map((entry) => (entry.entry === "action" ? entry.action.outcome : entry.entry))).toEqual([
      "said",
      "applied",
      "proposed",
      "reply",
    ]);
  });

  it("shows reports as kept text when the tab holds no turn for them, or one that answered otherwise", () => {
    const cases: [string, KeptTurn[]][] = [
      ["no kept turn", []],
      ["a turn kept at another time", [{ at: "2026-10-07T09:00:00Z", actions: [applied, proposed], ended: replied }]],
      ["a turn with a different count of writes", [{ at: "2026-10-07T10:00:05Z", actions: [applied], ended: replied }]],
    ];
    for (const [name, kept] of cases) {
      expect(
        transcript(messages, kept).map((entry) => entry.entry),
        name,
      ).toEqual(["said", "report", "report", "reply"]);
    }
  });

  it("shows how a turn ended without a reply after its writes", () => {
    const ended: Ended = { status: "provider_timed_out" };
    const cut = [said("2026-10-07T11:00:00Z", "Again."), report("2026-10-07T11:02:00Z", "Applied."), report("2026-10-07T11:02:00Z", "Ended.")];
    const entries = transcript(cut, [{ at: "2026-10-07T11:02:00Z", actions: [applied], ended }]);
    expect(entries.map((entry) => entry.entry)).toEqual(["said", "action", "ended"]);
  });
});

describe("the tab's kept turns", () => {
  it("dates a turn by its answer, never by the user's words alone", () => {
    expect(latestAnswerAt(messages)).toBe("2026-10-07T10:00:05Z");
    expect(latestAnswerAt(messages.slice(0, 1))).toBeUndefined();
    expect(latestAnswerAt([])).toBeUndefined();
  });

  it("keeps the newest turns, each time once", () => {
    let kept: KeptTurn[] = [];
    const count = KEPT_TURN_COUNT_MAX + 5;
    for (let index = 0; index < count; index += 1) {
      kept = keeping(kept, { at: `t${String(index)}`, actions: [], ended: replied });
    }
    kept = keeping(kept, { at: `t${String(count - 1)}`, actions: [applied], ended: replied });
    expect(kept).toHaveLength(KEPT_TURN_COUNT_MAX);
    expect(kept.at(-1)?.actions).toEqual([applied]);
    expect(kept[0]?.at).toBe(`t${String(count - KEPT_TURN_COUNT_MAX)}`);
  });
});

describe("a turn the host answered but may not have kept", () => {
  const answer = { conversation: "cv_one", actions: [applied, proposed], reply: "Done.", ended: replied };

  it("is found in the conversation by its words and the answer after them", () => {
    expect(holdsTurn(messages, "Kickoff happened; add a step.", answer)).toBe(true);
    expect(holdsTurn(messages.slice(0, 1), "Kickoff happened; add a step.", answer)).toBe(false);
    expect(holdsTurn(messages, "Something else.", answer)).toBe(false);
    expect(holdsTurn([], "Kickoff happened; add a step.", answer)).toBe(false);
  });

  it("shows from its answer alone: the words, each write, and the reply", () => {
    const entries = answeredEntries("Kickoff happened; add a step.", answer, "2026-10-07T10:00:05Z");
    expect(entries.map((entry) => (entry.entry === "action" ? entry.action.outcome : entry.entry))).toEqual(["said", "applied", "proposed", "reply"]);
  });
});
