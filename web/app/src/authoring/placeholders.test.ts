// A10, Keys and references: a message draft is written with roles by id and decisions by
// path, stored with keys, and survives a rename because it names keys; a placeholder that
// names nothing is caught before sending. The grammar is the engine's, so these ask it.
import type { Schema } from "@cairn/client";
import { beforeAll, describe, expect, it } from "vitest";
import type { Engine } from "@cairn/wasm";

import type { Role } from "./graph.ts";
import { treeOf, type GraphNode } from "./graph.ts";
import { seeded } from "./engine.test-support.ts";
import { draftProblemWords, placeholderOptions } from "./placeholders.ts";

const roles: Role[] = [{ key: "r_lead", id: "lead", title: "Lead" }];
const nodes: GraphNode[] = [
  { key: "n_testing", id: "testing", kind: "group", title: "Testing" },
  { key: "n_scope", id: "scope", kind: "decision", title: "Scope", prompt: "?", answer_type: "text", parent: "n_testing" },
];
const graph: Schema<"Graph"> = { nodes, roles };
const tree = treeOf(graph);

let engine: Engine;
beforeAll(async () => {
  engine = (await seeded()).engine;
});

const stored = (written: string, of = graph) => JSON.parse(engine.draftStoredText(of, written)) as { stored: string } | { problem: Parameters<typeof draftProblemWords>[0] };
const written = (text: string, of = graph) => JSON.parse(engine.draftWrittenText(of, text)) as string;

describe("message draft placeholders (A10)", () => {
  it("stores what an author writes with keys, and shows it back as written", () => {
    const text = "Hi {{roles.lead.name}}, about {{journey.name}}: {{answers.testing/scope}}.";
    const answer = stored(text);
    expect(answer).toEqual({ stored: "Hi {{roles.r_lead.name}}, about {{journey.name}}: {{answers.n_scope}}." });
    expect("stored" in answer ? written(answer.stored) : undefined).toBe(text);
  });

  it("keeps naming the same decision after it is renamed or moved", () => {
    const moved: Schema<"Graph"> = { nodes: [nodes[0] as GraphNode, { ...(nodes[1] as GraphNode), id: "breadth", parent: null }], roles };
    expect(written("{{answers.n_scope}}", moved)).toBe("{{answers.breadth}}");
  });

  it("refuses a placeholder that names nothing, or an unclosed one, and says so in words", () => {
    for (const text of ["{{roles.nobody.name}}", "{{answers.testing}}", "{{journey.colour}}", "Hi {{journey.name"]) {
      const answer = stored(text);
      expect(answer).toHaveProperty("problem");
      expect("problem" in answer && draftProblemWords(answer.problem)).not.toBe("");
    }
  });

  it("offers every journey field, role, and decision, each stored as it is offered", () => {
    const options = placeholderOptions(tree, roles);
    expect(options.length).toBeGreaterThan(0);
    for (const option of options) {
      expect(stored(option.written)).toHaveProperty("stored");
    }
  });
});
