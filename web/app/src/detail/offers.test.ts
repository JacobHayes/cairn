// C11, B5, B6, D4: what the inspector offers by kind and state, what its menu lists, what a snooze can set
// aside, and the roll-up a container's sentence opens with.
import { describe, expect, it } from "vitest";

import { nodeDetail, type NodeDerived, type Ready } from "./model.ts";
import { actMutation } from "./ActButton.tsx";
import { actLabel, contextMenuOf, menuOf, offered, unsnoozeOf } from "./offers.ts";
import { plainSentence, sentenceOf } from "./sentence.ts";
import { setAsideOptions } from "./snooze.ts";
import { actingView } from "../acting/acting.test-support.ts";
import { checklistView, testView } from "./view.test-support.ts";

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

describe("B6: where a snooze is offered", () => {
  it("is only where the engine says a snooze can sit, in the menu and in the choice of what to set aside", () => {
    const ids = (view: Ready) => menuOf(view, detailOf(view, "n_findings"), true).flat().map((item) => item.id);
    expect(ids(testView())).toContain("snooze");
    const held = made("n_findings", { snoozable: false });
    expect(ids(held)).not.toContain("snooze");
    expect(setAsideOptions(held, detailOf(held, "n_findings")).map((option) => option.node)).toEqual(["n_stage"]);
  });
});

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
    view.journey.graph.state = { ...view.journey.graph.state, nodes: { ...view.journey.graph.state?.nodes, n_when: { state: "decided" } } };
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
  });

  it("offers a placeholder to break down Break down, with Mark atomic beside it and no Mark done (B10)", () => {
    const view = made("n_findings", { needs_breakdown: true });
    expect(offered(view, "n_findings")).toEqual({ primary: "break-down", secondary: ["mark-atomic"] });
  });

  it("offers a decision awaiting its answer Decide, and nothing once it is decided", () => {
    expect(offered(testView(), "n_pick").primary).toBe("decide");
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

  it("offers Mark done with Start beside it, the same while something it requires is not done", () => {
    expect(offered(testView(), "n_findings")).toEqual({ primary: "done", secondary: ["start"] });
    const waits = made("n_findings", { display_state: "blocked", waits_on: { entries: [{ node: "n_meeting", via: "explicit" }], total: 1 } });
    expect(offered(waits, "n_findings")).toEqual({ primary: "done", secondary: ["start"] });
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

describe("G5: finishing work whose checklist is not all ticked", () => {
  it("offers Mark done... and Done anyway while an item is unticked, and Mark done alone once every one is", () => {
    const view = checklistView();
    const menu = () => menuOf(view, detailOf(view, "n_findings"), true).flat().map((item) => item.id);
    expect(offered(view, "n_findings").primary).toBe("done-with-evidence");
    expect(menu()).toContain("done-anyway");
    const ticked = checklistView(["a", "b", "c"]);
    expect(offered(ticked, "n_findings").primary).toBe("done");
    expect(menuOf(ticked, detailOf(ticked, "n_findings"), true).flat().map((item) => item.id)).not.toContain("done-anyway");
  });
});

describe("D8: a node waiting for its date", () => {
  it("offers Mark done and Start on work before its own date, as plain moves (D8)", () => {
    const view = made("n_findings", { display_state: "scheduled", dates: { not_before: "2026-11-02" } });
    const acts = contextMenuOf(view, detailOf(view, "n_findings"), true).flat().map((item) => item.id);
    expect(acts).toContain("done");
    expect(acts).toContain("start");
    expect(acts).not.toContain("done-anyway");
  });

  it("says the date a scheduled node waits for, by kind (D8)", () => {
    const say = (view: Ready, key: string) => plainSentence(view, sentenceOf(view, detailOf(view, key), { position: undefined, row: undefined }));
    const work = made("n_findings", { display_state: "scheduled", dates: { not_before: "2026-11-02" } });
    expect(say(work, "n_findings")).toContain("Nov 2");
    const meeting = made("n_meeting", { display_state: "scheduled" });
    expect(say(meeting, "n_meeting")).toContain("Nov 20");
    expect(say(meeting, "n_meeting")).not.toBe(say(work, "n_findings"));
  });

  it("names the date of a scheduled node a blocked node waits on (D8)", () => {
    const say = (view: Ready, key: string) => plainSentence(view, sentenceOf(view, detailOf(view, key), { position: undefined, row: undefined }));
    const blocked = made("n_report", { display_state: "blocked", waits_on: { entries: [{ node: "n_findings", via: "explicit", date: "2026-11-02" }], total: 1 } });
    expect(say(blocked, "n_report")).toContain("Nov 2");
  });
});

describe("a group marked done (D1a)", () => {
  const ids = (view: Ready, key: string) => menuOf(view, detailOf(view, key), true).flat().map((item) => item.id);

  /** The stage stored as `state`, with its report effectively skipped through it when it is marked done. */
  function stage(state: "derived" | "done", derived: Partial<NodeDerived> = {}): Ready {
    const view = made("n_stage", state === "done" ? { moves: ["insert_segment", "reopen"], skips_beneath: true, ...derived } : derived);
    view.journey.graph.state = { ...view.journey.graph.state, nodes: { ...view.journey.graph.state?.nodes, n_stage: { state, ...(state === "done" ? { reason: "Covered elsewhere." } : {}) } } };
    return view;
  }

  it("offers an open stage Mark done before Skip, and a stage that reads done or skipped neither", () => {
    expect(ids(stage("derived"), "n_stage").slice(0, 3)).toEqual(["snooze", "mark-done", "skip"]);
    expect(ids(made("n_stage", { display_state: "done", open: false }), "n_stage")).not.toContain("mark-done");
    expect(ids(made("n_stage", { display_state: "skipped", open: false, skipped_by: "n_meeting" }), "n_stage")).not.toContain("mark-done");
    expect(ids(made("n_findings", {}), "n_findings")).not.toContain("mark-done");
  });

  it("offers a stage marked done a plain Reopen first, also when an answer has since ruled it out", () => {
    const done = stage("done", { display_state: "done", open: false });
    expect(ids(done, "n_stage")[0]).toBe("reopen");
    expect(ids(done, "n_stage")).not.toContain("mark-done");
    const ruledOut = stage("done", { display_state: "not_relevant", open: false });
    expect(ids(ruledOut, "n_stage").slice(0, 2)).toEqual(["reopen", "include"]);
  });

  it("says how many items it skipped and which kept work is pending, as the pieces and the count", () => {
    const view = stage("done", { display_state: "done", open: false, kept_work_pending: ["n_findings"], skip_count: 1 });
    const report = view.derived.nodes.n_report;
    if (report !== undefined) {
      view.derived.nodes.n_report = { ...report, display_state: "skipped", open: false, skipped_by: "n_stage" };
    }
    const pieces = sentenceOf(view, detailOf(view, "n_stage"), { position: undefined, row: undefined });
    expect(pieces).toContainEqual({ node: "n_findings" });
    expect(plainSentence(view, pieces)).toContain("1 unfinished item");
    const through = sentenceOf(view, detailOf(view, "n_report"), { position: undefined, row: undefined });
    expect(through).toContainEqual({ node: "n_stage" });
  });
});

describe("what the engine says a node accepts, read as it derived it", () => {
  const view = actingView();
  const ids = (key: string) => menuOf(view, detailOf(view, key), true).flat().map((item) => item.id);

  it("offers Reopen on finished work an answer has since ruled out, with Include anyway", () => {
    expect(ids("n_closed_ruled").slice(0, 2)).toEqual(["reopen", "include"]);
  });

  it("offers Insert segment on work that holds nothing yet, and on no decision or milestone", () => {
    expect(ids("n_plain")).toContain("insert-segment");
    expect(ids("n_pick")).not.toContain("insert-segment");
    expect(ids("n_meet")).not.toContain("insert-segment");
  });

  it("offers work that no longer applies neither Start nor Mark done", () => {
    const offers = offered(view, "n_ruled");
    expect(offers.primary).toBeUndefined();
    expect(offers.secondary).not.toContain("start");
  });

  it("offers Mark done with its evidence form where a guard fails, and Done anyway beside it", () => {
    expect(offered(view, "n_log").primary).toBe("done-with-evidence");
    expect(ids("n_log")).toContain("done-anyway");
    expect(offered(view, "n_plain").primary).toBe("done");
    expect(ids("n_plain")).not.toContain("done-anyway");
  });
});
