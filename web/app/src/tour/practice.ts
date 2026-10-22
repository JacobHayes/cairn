// The practice journey the tour makes for a person: one patch that creates it and fills it, so
// the tour walks the ordinary screens over a real journey (decisions/2026-10-22-the-tour-practises-on-a-real-journey.md).
// The graph is what the tour's words rely on: a decision that applies to one deliverable, one
// piece of work whose completion frees two others, and a milestone whose pin gives the last
// deliverable its date. The engine derives every state shown; nothing here is hand drawn.
import type { Mutation } from "../data/writes.ts";

/** What the practice journey is called, and how it says where it came from. */
export const PRACTICE_NAME = "Practice: a short review";
export const PRACTICE_DESCRIPTION = "Made by the tour of Cairn. It is deleted when the tour ends.";

/** The practice journey's nodes, by what the tour says of them. */
export const PRACTICE = {
  decision: "n_outside",
  draft: "n_draft",
  comments: "n_comments",
  review: "n_review",
  summary: "n_summary",
  meeting: "n_meeting",
} as const;

/** How many days after today the meeting is pinned. */
export const MEETING_DAYS_AWAY = 14;

/** `date` (YYYY-MM-DD) moved by `days` calendar days. */
export function addDays(date: string, days: number): string {
  const [year, month, day] = date.split("-").map(Number);
  const moved = new Date(Date.UTC(year ?? 1970, (month ?? 1) - 1, (day ?? 1) + days));
  return moved.toISOString().slice(0, 10);
}

/** The mutations that create the practice journey and fill it, with the meeting pinned `MEETING_DAYS_AWAY` after `today`, the deployment's date. */
export function practiceMutations(today: string): Mutation[] {
  const { decision, draft, comments, review, summary, meeting } = PRACTICE;
  return [
    { op: "create_journey", name: PRACTICE_NAME, description: PRACTICE_DESCRIPTION },
    { op: "add_node", node: { key: decision, id: "outside", kind: "decision", title: "Does an outside reviewer look at this?", answer_type: "boolean", prompt: "Does an outside reviewer look at this?" } },
    { op: "add_node", node: { key: draft, id: "draft", kind: "deliverable", title: "Write the draft", estimate: 3 } },
    { op: "add_node", node: { key: comments, id: "comments", kind: "action", title: "Collect comments", estimate: 2 } },
    {
      op: "add_node",
      node: { key: review, id: "review", kind: "deliverable", title: "Outside review", estimate: 3, relevant_when: { equals: { decision, value: true } } },
    },
    { op: "add_node", node: { key: summary, id: "summary", kind: "deliverable", title: "Write the summary", estimate: 2, due_by: { before: meeting, offset: 1 } } },
    { op: "add_node", node: { key: meeting, id: "meeting", kind: "milestone", title: "Review meeting", final: true } },
    { op: "add_edge", edge: { node: comments, requires: draft } },
    { op: "add_edge", edge: { node: review, requires: draft } },
    { op: "add_edge", edge: { node: summary, requires: comments } },
    { op: "add_edge", edge: { node: summary, requires: review } },
    { op: "add_edge", edge: { node: meeting, requires: summary } },
    { op: "set_pin", node: meeting, date: addDays(today, MEETING_DAYS_AWAY) },
  ];
}
