// C8: a node's Contents checks work off in place, and only where the engine accepts the move.
import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";

import { builtJourney, loadedEngine } from "../authoring/engine.test-support.ts";
import type { Mutation } from "../authoring/graph.ts";
import { ContentsRow } from "./Contents.tsx";
import { nodeDetail } from "./model.ts";
import type { NodeWrite } from "./write.ts";

await loadedEngine();

const add = (key: string, kind: string, extra: Record<string, unknown> = {}) =>
  ({ op: "add_node", node: { key, id: key.slice(2), kind, title: key, ...(kind === "decision" ? { prompt: key, answer_type: "boolean" } : {}), ...extra } }) as Mutation;

const view = builtJourney("j_contents", "Contents", [
  add("n_gate", "decision"),
  add("n_box", "group"),
  add("n_plain", "action", { parent: "n_box" }),
  add("n_finished", "action", { parent: "n_box" }),
  add("n_dropped", "action", { parent: "n_box" }),
  add("n_ruled", "action", { parent: "n_box", relevant_when: { not: { answered: "n_gate" } } }),
  add("n_needs_note", "action", { parent: "n_box", requires_note: true }),
  { op: "transition", node: "n_finished", transition: "complete" },
  { op: "transition", node: "n_dropped", transition: { skip: { reason: "Not needed." } } },
  { op: "answer", decision: "n_gate", value: { boolean: false } },
] as Mutation[]);

const write = { writable: true, disabled: false, run: () => Promise.resolve(true) } as unknown as NodeWrite;

/** The checkbox label of each child of the box, or none when the row has no checkbox. */
function labels(): Record<string, string | undefined> {
  const detail = nodeDetail(view, "n_box");
  return Object.fromEntries(
    (detail?.children ?? []).map((child) => {
      const markup = renderToStaticMarkup(
        <MemoryRouter>
          <ContentsRow view={view} child={child} write={write} viewer={undefined} />
        </MemoryRouter>,
      );
      return [child.key, /aria-label="([^"]*)"/.exec(markup)?.[1]];
    }),
  );
}

describe("the Contents checkbox", () => {
  it("completes or reopens only where the engine accepts it, and is absent for work it would refuse", () => {
    expect(labels()).toEqual({
      n_plain: "Complete n_plain",
      n_finished: "Reopen n_finished",
      n_dropped: undefined,
      // Ruled out by an answer: a completion is refused.
      n_ruled: undefined,
      // Its guard fails: the detail's Mark done... takes the evidence first.
      n_needs_note: undefined,
    });
  });
});
