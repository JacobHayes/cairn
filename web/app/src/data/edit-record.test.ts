// H5: what the tab keeps of an edit is what may have committed, and only what this build can
// resume.
import { describe, expect, it } from "vitest";

import { EDIT_RECORD_VERSION, readRecord, tabEditStore, type EditRecord } from "./edit-record.ts";
import { memoryStorage } from "./fake.test-support.ts";

const record = (id: string, delivery: EditRecord["delivery"] = { state: "sending" }): EditRecord => ({
  version: EDIT_RECORD_VERSION,
  id,
  command: { kind: "patch", patch: { id, target: { journey: "j_one" }, base_revision: 3, mutations: [{ op: "set_node_field", node: "n_a", value: { title: "Renamed" } }] } },
  about: { edit: "title:j_one:n_a", label: "Draft the plan", address: "/journeys/j_one/n_a" },
  delivery,
  resubmitted: 2,
});

describe("the tab's edit records", () => {
  it("come back as they were kept, every kind of delivery, and go when forgotten", () => {
    const held = memoryStorage();
    const kept = tabEditStore(() => held);
    const records = [
      record("p_a"),
      record("p_b", { state: "unconfirmed", failure: { status: 0, message: "down" } }),
      record("p_c", { state: "rejected", rejection: { rejection: "invalid", violations: [] } }),
      record("p_d", { state: "stopped" }),
      record("p_e", { state: "gone", removed: { node: "n_a" } }),
    ];
    expect(kept.read()).toEqual({ records: [], dropped: 0 });
    for (const each of records) {
      expect(kept.keep(each)).toBe(true);
    }
    expect(kept.read().records).toEqual(expect.arrayContaining(records));
    expect(kept.read().records).toHaveLength(records.length);
    kept.forget("p_a");
    expect(kept.read().records.map((each) => each.id).sort()).toEqual(["p_b", "p_c", "p_d", "p_e"]);
  });

  it("drop, and remove, what this build cannot resume, counting each and keeping the rest", () => {
    const held = memoryStorage();
    const store = tabEditStore(() => held);
    store.keep(record("p_good"));
    held.setItem("cairn:edit:p_old", JSON.stringify({ ...record("p_old"), version: EDIT_RECORD_VERSION + 1 }));
    held.setItem("cairn:edit:p_broken", "{not json");
    held.setItem("cairn:edit:p_mismatch", JSON.stringify(record("p_other")));
    held.setItem("cairn:draft:title:j_one:n_a", "{}");
    const read = store.read();
    expect(read.records.map((each) => each.id)).toEqual(["p_good"]);
    expect(read.dropped).toBe(3);
    expect(held.getItem("cairn:edit:p_old")).toBeNull();
    expect(held.getItem("cairn:edit:p_broken")).toBeNull();
    expect(held.getItem("cairn:edit:p_mismatch")).toBeNull();
    expect(held.getItem("cairn:draft:title:j_one:n_a")).not.toBeNull();
    expect(store.read().dropped).toBe(0);
  });

  it("report a refusal by storage as not kept, and read as empty where there is no storage", () => {
    const refusing: Storage = Object.assign(memoryStorage(), {
      setItem: () => {
        throw new DOMException("full", "QuotaExceededError");
      },
    });
    expect(tabEditStore(() => refusing).keep(record("p_a"))).toBe(false);
    const none = tabEditStore(() => undefined);
    expect(none.keep(record("p_a"))).toBe(false);
    expect(none.read()).toEqual({ records: [], dropped: 0 });
  });
});

const proposalRecord = (id: string, delivery: EditRecord["delivery"] = { state: "unconfirmed" }): EditRecord => ({
  ...record(id, delivery),
  command: { kind: "proposal", call: { op: "apply", destination: { journey: "j_one" }, proposal: "pr_one", request: { patch_id: id, reviewed_revision: 4 } } },
});
const importRecord = (id: string): EditRecord => ({
  ...record(id),
  command: { kind: "import", request: { patch_id: id, file: { format: 1, route: "r_one", name: "One", nodes: [] } as never } },
});

describe("the records of proposal writes and imports", () => {
  it("come back as they were kept, with the call or the file a resend needs", () => {
    const held = memoryStorage();
    const kept = tabEditStore(() => held);
    const records = [proposalRecord("p_a"), proposalRecord("p_b", { state: "rejected", rejection: { rejection: "invalid", violations: [] } }), importRecord("p_c")];
    for (const each of records) {
      expect(kept.keep(each)).toBe(true);
    }
    expect(kept.read().records).toEqual(expect.arrayContaining(records));
    expect(kept.read().records).toHaveLength(records.length);
  });

  it("are dropped when their request is not the one the record is named by, or lacks what it needs", () => {
    const apply = proposalRecord("p_a");
    const cases: unknown[] = [
      { ...apply, command: { kind: "proposal", call: { op: "apply", destination: { journey: "j_one" }, proposal: "pr_one", request: { patch_id: "p_other", reviewed_revision: 4 } } } },
      { ...apply, command: { kind: "proposal", call: { op: "apply", request: { patch_id: "p_a", reviewed_revision: 4 } } } },
      { ...apply, command: { kind: "proposal", call: { op: "explode", proposal: "pr_one", request: { patch_id: "p_a" } } } },
      { ...apply, command: { kind: "proposal", call: { op: "apply", proposal: "pr_one" } } },
      // A write to a proposal names the destination it is under: without one it has no address to be sent to.
      { ...apply, command: { kind: "proposal", call: { op: "apply", proposal: "pr_one", request: { patch_id: "p_a", reviewed_revision: 4 } } } },
      { ...importRecord("p_a"), command: { kind: "import", request: { patch_id: "p_a" } } },
      { ...importRecord("p_a"), command: { kind: "import", request: { patch_id: "p_other", file: {} } } },
    ];
    expect(cases.map(readRecord)).toEqual(cases.map(() => undefined));
  });

  it("are not read from a record of the version before them", () => {
    const held = memoryStorage();
    held.setItem("cairn:edit:p_a", JSON.stringify({ ...proposalRecord("p_a"), version: EDIT_RECORD_VERSION - 1 }));
    expect(tabEditStore(() => held).read()).toEqual({ records: [], dropped: 1 });
  });
});

describe("readRecord", () => {
  it("accepts only the one version, and a delivery and command of a known shape", () => {
    expect(readRecord(record("p_a"))).toEqual(record("p_a"));
    const cases: unknown[] = [
      undefined,
      "text",
      { ...record("p_a"), version: 0 },
      { ...record("p_a"), delivery: { state: "landed" } },
      { ...record("p_a"), delivery: { state: "rejected", rejection: { rejection: "unknown" } } },
      { ...record("p_a"), command: { kind: "proposal" } },
      { ...record("p_a"), resubmitted: -1 },
      { ...record("p_a"), about: { edit: 3 } },
    ];
    expect(cases.map(readRecord)).toEqual(cases.map(() => undefined));
  });
});
