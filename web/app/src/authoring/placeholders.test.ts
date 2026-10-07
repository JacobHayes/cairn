// A10, Identity and references: a message draft is written with roles by id and decisions by
// path, stored with keys, and survives a rename because it names keys; a placeholder that
// names nothing is caught before sending.
import { describe, expect, it } from "vitest";

import { treeOf, type GraphNode, type Role } from "./graph.ts";
import { placeholderOptions, toStored, toWritten } from "./placeholders.ts";

const roles: Role[] = [{ key: "r_lead", id: "lead", title: "Lead" }];
const nodes: GraphNode[] = [
  { key: "n_testing", id: "testing", kind: "group", title: "Testing" },
  { key: "n_scope", id: "scope", kind: "decision", title: "Scope", prompt: "?", answer_type: "text", parent: "n_testing" },
];
const tree = treeOf({ nodes, roles });

describe("message draft placeholders (A10)", () => {
  it("stores what an author writes with keys, and shows it back as written", () => {
    const written = "Hi {{roles.lead.name}}, about {{journey.name}}: {{answers.testing/scope}}.";
    const stored = toStored(written, tree, roles);
    expect(stored).toEqual({ stored: "Hi {{roles.r_lead.name}}, about {{journey.name}}: {{answers.n_scope}}." });
    expect("stored" in stored ? toWritten(stored.stored, tree, roles) : undefined).toBe(written);
  });

  it("keeps naming the same decision after it is renamed or moved", () => {
    const moved = treeOf({ nodes: [nodes[0] as GraphNode, { ...(nodes[1] as GraphNode), id: "breadth", parent: null }], roles });
    expect(toWritten("{{answers.n_scope}}", moved, roles)).toBe("{{answers.breadth}}");
  });

  it("refuses a placeholder that names nothing, or an unclosed one", () => {
    expect(toStored("{{roles.nobody.name}}", tree, roles)).toHaveProperty("problem");
    expect(toStored("{{answers.testing}}", tree, roles)).toHaveProperty("problem");
    expect(toStored("{{journey.colour}}", tree, roles)).toHaveProperty("problem");
    expect(toStored("Hi {{journey.name", tree, roles)).toHaveProperty("problem");
  });

  it("offers every journey field, role, and decision, each stored as it is offered", () => {
    for (const option of placeholderOptions(tree, roles)) {
      expect(toStored(option.written, tree, roles)).toHaveProperty("stored");
    }
  });
});
