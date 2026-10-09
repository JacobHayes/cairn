// C11, B5, B6, D4: what the inspector offers by kind and state, what its menu lists, what a snooze can set
// aside, and the roll-up a container's sentence opens with.
import { describe, expect, it } from "vitest";

import { nodeDetail, type NodeDerived, type Ready } from "./model.ts";
import { menuOf, offered } from "./offers.ts";
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

describe("the inspector's offers", () => {
  it("lists a decided decision's Reopen and the shape of the node, with nothing to snooze or skip or answer", () => {
    const view = made("n_when", { display_state: "done" });
    view.journey.graph.state = { ...view.journey.graph.state, nodes: { ...view.journey.graph.state?.nodes, n_when: { state: "decided", provenance: "local" } } };
    expect(menuOf(view, detailOf(view, "n_when")).map((group) => group.map((item) => item.id))).toEqual([["reopen"], ["rename", "assign", "weight"], ["show-in-graph", "edit-node", "copy-link"]]);
    expect(offered(view, detailOf(view, "n_when")).primary).toBeUndefined();
  });

  it("offers Mark done with Start beside it, which waits when something it requires is not done", () => {
    expect(offered(testView(), detailOf(testView(), "n_findings"))).toEqual({ primary: "done", secondary: ["start"], waiting: false });
    expect(offered(testView(), detailOf(testView(), "n_report")).waiting).toBe(true);
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
