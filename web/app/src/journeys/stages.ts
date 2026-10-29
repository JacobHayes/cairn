// The journey card's Stages list: one row per stage (a top-level group), in the order the work runs
// (by due date, the stages with none after, then as the document lists them: the canvas lays stages out
// by what they wait on, which their dates follow), each with what the stage's own card says: its
// state, how much is done and when it is due.
import { inkClass, dateInk } from "../status/when.ts";
import { dueWordsOf, isStage, recordOf, titleOf, type Ready } from "../detail/model.ts";
import type { DisplayState } from "../status/words.ts";

export interface StageRow {
  key: string;
  title: string;
  displayState: DisplayState;
  done: number;
  total: number;
  /** When it is due, in words; none once it is finished. */
  due: string | undefined;
  /** The class that says how near the due date is (error once late, warning when soon), as the card foot does. */
  dueInk: string | undefined;
}

/** The journey's stages as rows, in the order the work runs; none when it has no stage. */
export function stagesOf(view: Ready): StageRow[] {
  const dueOf = (key: string) => view.derived.nodes[key]?.dates.due?.date ?? "9999-12-31";
  const stages = (view.journey.graph.nodes ?? []).filter(isStage);
  // The sort is stable, so equal dates keep the document's order.
  return stages.toSorted((one, other) => dueOf(one.key).localeCompare(dueOf(other.key))).map((node) => {
    const derived = view.derived.nodes[node.key];
    const displayState = derived?.display_state ?? "ready";
    const { done, total } = derived?.progress ?? { done: 0, total: 0 };
    const due = dueWordsOf(view, { key: node.key, kind: node.kind, state: recordOf(view, node).state, displayState });
    const date = due === undefined ? undefined : derived?.dates.due?.date;
    return { key: node.key, title: titleOf(view, node.key), displayState, done, total, due, dueInk: date === undefined ? undefined : inkClass(dateInk(date, view.derived.today)) };
  });
}
