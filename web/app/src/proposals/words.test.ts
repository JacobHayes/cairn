// A proposal's changes in words (C14): a person reads titles, labels, yes or no and dates,
// never an id, a key or a field's internal name.
import { describe, expect, it } from "vitest";

import { conditionWords } from "../authoring/condition.ts";
import type { Graph, Mutation } from "../authoring/graph.ts";
import { mutationWords, namesOf, outlineWords, stateValueWords } from "./words.ts";

const graph: Graph = {
  nodes: [
    { key: "n_who", id: "who", kind: "decision", title: "Who runs testing?", answer_type: "single_choice", choices: [{ id: "partner_team", title: "Partner" }, "ours"] },
    { key: "n_ok", id: "ok", kind: "decision", title: "Approved?", answer_type: "boolean" },
    { key: "n_test", id: "test", kind: "task", title: "Run the tests", checklist: [{ id: "load", title: "Load test" }] },
  ],
} as Graph;

const names = namesOf([graph], { revision: 1, identities: [{ key: "who_ann", name: "Ann" }] });

describe("mutationWords", () => {
  const cases: [string, Mutation, string[], string[]][] = [
    ["a choice by its label", { op: "answer", decision: "n_who", value: { single_choice: "partner_team" } }, ["Who runs testing?", "Partner"], ["partner_team"]],
    ["a boolean as yes or no", { op: "answer", decision: "n_ok", value: { boolean: true } }, ["Approved?", "yes"], ["true"]],
    ["a pin as a date in words", { op: "set_pin", node: "n_test", date: "2026-11-25" }, ["Run the tests", "Nov 25"], ["2026-11-25"]],
    ["a rule by what it is", { op: "set_node_field", node: "n_test", value: { due_by: { offset: 3 } } }, ["Run the tests", "due"], ["due_by"]],
    ["a tick by its item's title", { op: "set_ticked", node: "n_test", item: "load", ticked: true }, ["Run the tests", "Load test"], ["load\""]],
    ["a transition as a verb", { op: "transition", node: "n_test", transition: "complete" }, ["Complete", "Run the tests"], []],
  ];
  it.each(cases)("says %s", (_, mutation, shown, hidden) => {
    const words = mutationWords(mutation, names);
    for (const part of shown) {
      expect(words).toContain(part);
    }
    for (const part of hidden) {
      expect(words).not.toContain(part);
    }
  });

  it("names the journey's start as a rule's source, never as a removed item", () => {
    const words = mutationWords({ op: "set_node_field", node: "n_test", value: { due_by: { after: "journey.created_at", offset: 3 } } }, names);
    expect(words).toContain("journey's start");
    expect(words).not.toContain("removed");
  });

  it("says a condition with the decision's title and the choice's label", () => {
    const words = mutationWords({ op: "set_node_field", node: "n_test", value: { relevant_when: { equals: { decision: "n_who", value: "partner_team" } } } }, names);
    expect(words).toContain("Who runs testing?");
    expect(words).toContain("Partner");
    expect(words).not.toContain("partner_team");
  });
});

describe("conditionWords", () => {
  it("names each compared value of a combined condition by its decision's choices", () => {
    const words = conditionWords({ any: [{ equals: { decision: "n_who", value: "partner_team" } }, { not_equals: { decision: "n_ok", value: false } }] }, names.condition);
    expect(words).toContain("Partner");
    expect(words).toContain("is not no");
    expect(words).not.toContain("partner_team");
    expect(words).not.toContain("false");
  });
});

describe("outlineWords (C14)", () => {
  it("names each count the outline holds and none it does not", () => {
    const words = outlineWords({ nodes_added: 3, requirements_removed: 1 });
    expect(words).toContain("3");
    expect(words).toContain("1");
    expect(words).not.toContain("changes");
  });

  it("says a proposal with nothing in it changes nothing, whatever it is asked for twice", () => {
    expect(outlineWords({})).toBe(outlineWords({ nodes_added: 0 }));
    expect(outlineWords({})).not.toBe(outlineWords({ nodes_added: 1 }));
  });

  it("puts a lifecycle fact first, before the nodes", () => {
    const words = outlineWords({ facts: [{ fact: "opens_draft" }], nodes_added: 1 });
    expect(words.indexOf("opens")).toBeLessThan(words.indexOf("1"));
  });
});

describe("stateValueWords (C14)", () => {
  it("says the ticked items by their titles, and nothing ticked in words", () => {
    expect(stateValueWords("ticks", ["load"], "n_test", names)).toContain("Load test");
    expect(stateValueWords("ticks", [], "n_test", names)).not.toBe("");
  });

  const cases: [string, string, unknown, string[], string[]][] = [
    ["an identity answer by the identity's name", "answers", { identity: "who_ann" }, ["Ann"], ["who_ann", "{"]],
    ["a choice answer by its label", "answers", { single_choice: "partner_team" }, ["Partner"], ["partner_team", "{"]],
    ["a snooze on a node by its title", "snoozes", { node: "n_test" }, ["Run the tests"], ["n_test", "{"]],
    ["a snooze on a date in words", "snoozes", { date: "2026-11-25" }, ["Nov 25"], ["2026-11-25", "{"]],
    ["a pin as a date in words", "pins", "2026-11-25", ["Nov 25"], ["2026-11-25"]],
    ["a node's stored state as what it is", "nodes", { state: "in_progress", started_on: "2026-11-25" }, ["in progress", "started"], ["{", "provenance", "2026-11-25"]],
  ];
  it.each(cases)("says %s", (_, field, value, shown, hidden) => {
    const words = stateValueWords(field, value, "n_who", names);
    for (const part of shown) {
      expect(words).toContain(part);
    }
    for (const part of hidden) {
      expect(words).not.toContain(part);
    }
  });
});
