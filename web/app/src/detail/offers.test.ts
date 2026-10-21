// C11, B5, B6, D4: what the inspector offers by kind and state, what its menu lists, what a snooze can set
// aside, and the roll-up a container's sentence opens with.
import { describe, expect, it } from "vitest";

import { nodeDetail, type NodeDerived, type Ready } from "./model.ts";
import { actMutation } from "./ActButton.tsx";
import { actLabel, contextMenuOf, menuOf, offered, unsnoozeOf } from "./offers.ts";
import { plainSentence, sentenceOf } from "./sentence.ts";
import { setAsideOptions } from "./snooze.ts";
import { testView } from "./view.test-support.ts";

/** `view` with node `key`'s derived values changed. */
function made(key: string, derived: Partial<NodeDerived>): Ready {
  const view = testView();
  const found = view.derived.nodes[key];
  if (found !== undefined) {
    view.derived.nodes[key] = { ...found, ...derived };
  }
  return view;
}

function detailOf(view: Ready, key: string) {
  const detail = nodeDetail(view, key);
  if (detail === undefined) {
    throw new Error(`no ${key}`);
  }
  return detail;
}

describe("a page that takes no write (B11)", () => {
  it("offers only where to find a node on a page that takes no write, on the menu and the graph's context menu", () => {
    const view = made("n_findings", {});
    expect(menuOf(view, detailOf(view, "n_findings"), false).flat().map((item) => item.id)).toEqual(["show-in-graph", "copy-link"]);
    expect(contextMenuOf(view, detailOf(view, "n_findings"), false).flat().map((item) => item.id)).toEqual(["show-in-list", "copy-link"]);
  });
});

describe("the inspector's offers", () => {
  it("lists a decided decision's Reopen and the shape of the node, with nothing to snooze or skip or answer", () => {
    const view = made("n_when", { display_state: "done" });
    view.journey.graph.state = { ...view.journey.graph.state, nodes: { ...view.journey.graph.state?.nodes, n_when: { state: "decided", provenance: "local" } } };
    expect(menuOf(view, detailOf(view, "n_when"), true).map((group) => group.map((item) => item.id))).toEqual([["reopen"], ["rename", "assign", "weight"], ["show-in-graph", "edit-node", "copy-link"]]);
    expect(offered(view, "n_when").primary).toBeUndefined();
  });

  it("opens the graph's context menu with the row's actions, then the menu's groups, finding the node in the list not the graph it is on", () => {
    const view = testView();
    const groups = contextMenuOf(view, detailOf(view, "n_findings"), true).map((group) => group.map((item) => item.id));
    expect(groups[0]).toEqual(["done", "start"]);
    expect(groups.flat()).toContain("show-in-list");
    expect(groups.flat()).not.toContain("show-in-graph");
    expect(groups.at(-1)?.at(-1)).toBe("copy-link");
    // Waiting on a requirement, Mark done is left to Done anyway.
    const waiting = contextMenuOf(view, detailOf(view, "n_report"), true).flat().map((item) => item.id);
    expect(waiting).not.toContain("done");
    expect(waiting).toContain("done-anyway");
  });

  it("offers a placeholder to break down Break down, with Mark atomic beside it and no Mark done (B10)", () => {
    const view = made("n_findings", { needs_breakdown: true });
    expect(offered(view, "n_findings")).toEqual({ primary: "break-down", secondary: ["mark-atomic"], waiting: false });
  });

  it("offers a decision awaiting its answer Decide, and nothing once it is decided", () => {
    expect(offered(testView(), "n_when").primary).toBe("decide");
    const decided = made("n_when", { display_state: "done" });
    expect(offered(decided, "n_when").primary).toBeUndefined();
  });

  it("offers Unsnooze of the container holding a node that has no snooze of its own (B6), and says whose", () => {
    const view = made("n_findings", { display_state: "snoozed", snoozed_via: "n_stage" });
    expect(offered(view, "n_findings").secondary).toEqual(["start", "unsnooze"]);
    expect(unsnoozeOf(view, "n_findings")).toBe("n_stage");
    expect(actMutation(view, "unsnooze", "n_findings")).toEqual({ op: "unsnooze", node: "n_stage" });
    expect(actLabel(view, "n_findings", "unsnooze")).toContain("Stage");
    expect(unsnoozeOf(testView(), "n_findings")).toBeUndefined();
    // A container with a snooze of its own lifts that before the one above it.
    view.journey.graph.state = { ...view.journey.graph.state, snoozes: { n_findings: { date: "2026-12-01" } } };
    expect(unsnoozeOf(view, "n_findings")).toBe("n_findings");
  });

  it("names a scheduled milestone's Mark done the same on the row and in the graph's menu", () => {
    const view = made("n_meeting", { display_state: "scheduled" });
    const menu = contextMenuOf(view, detailOf(view, "n_meeting"), true)[0] ?? [];
    expect(offered(view, "n_meeting").primary).toBe("done");
    expect(menu.map((item) => item.label)).toContain(actLabel(view, "n_meeting", "done"));
  });

  it("offers Mark done with Start beside it, which waits when something it requires is not done", () => {
    expect(offered(testView(), "n_findings")).toEqual({ primary: "done", secondary: ["start"], waiting: false });
    expect(offered(testView(), "n_report").waiting).toBe(true);
  });

  it("offers a snooze the node, then each container above it with its open count", () => {
    const view = testView();
    expect(setAsideOptions(view, detailOf(view, "n_findings"))).toEqual([
      { node: "n_findings", title: "Findings", open: undefined },
      { node: "n_stage", title: "Stage", open: 2 },
    ]);
  });

  it("opens a container's sentence with what is done and the open child with the least slack, or the one that is late", () => {
    const say = (view: Ready) => plainSentence(view, sentenceOf(view, detailOf(view, "n_stage"), { position: undefined, row: undefined }));
    expect(say(testView())).toBe("0 of 2 done; the tightest item, Report, has 24 days to spare. In progress.");
    expect(say(made("n_report", { dates: { slack_days: -2 } }))).toBe("0 of 2 done; 2 days past the latest start of Report. In progress.");
  });
});
