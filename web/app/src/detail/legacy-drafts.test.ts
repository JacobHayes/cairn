// ARCHITECTURE, Web UI: drafts survive a reload, even the one a deploy causes. A tab open across
// the change to one actions draft per node and the edit manager's records holds the old keys:
// they are converted once, nothing a person typed is lost, and what cannot be read goes.
import { describe, expect, it } from "vitest";

import { startRoot } from "../data/edits.test-support.ts";
import { tabEditStore, type EditStore } from "../data/edit-record.ts";
import { memoryStorage } from "../data/fake.test-support.ts";
import { writeView } from "../data/write-machine.ts";
import { opening } from "./actions-form.ts";
import { migrateTabDrafts } from "./legacy-drafts.ts";

const P = "cairn:draft:";
const drafts = (storage: Storage, entries: Record<string, unknown>): void => {
  for (const [key, value] of Object.entries(entries)) {
    storage.setItem(P + key, typeof value === "string" && value.startsWith("RAW:") ? value.slice(4) : JSON.stringify(value));
  }
};
const read = (storage: Storage, key: string): unknown => JSON.parse(storage.getItem(P + key) ?? "null");
const keysOf = (storage: Storage): string[] => Array.from({ length: storage.length }, (_, at) => storage.key(at) ?? "").sort();
const old = <T>(value: T, base = 3, deployment = 1) => ({ value, base, deployment });
const SNOOZE = { aside: "n_a", until: "date", date: "2026-10-20", target: "" };

const migrate = (storage: Storage, store: EditStore = tabEditStore(() => storage)): void => {
  migrateTabDrafts(storage, store);
};

describe("an old per-form draft", () => {
  it("becomes the node's one draft, choosing the form the old screen showed first, and the old keys go", () => {
    const storage = memoryStorage();
    drafts(storage, { "skip:j_one:n_a": old("Waiting on the vendor"), "snooze:j_one:n_a": old(SNOOZE), "title:j_one:n_a": { text: "Unrelated" } });
    migrate(storage);
    expect(read(storage, "actions:j_one:n_a")).toEqual(opening("skip", "Waiting on the vendor", { base: 3, deployment: 1 }));
    expect(keysOf(storage)).toEqual([`${P}actions:j_one:n_a`, `${P}title:j_one:n_a`]);
  });

  it("keeps each node's own draft, from every old key", () => {
    const storage = memoryStorage();
    drafts(storage, {
      "keep:j_one:n_a": old("Needed"),
      "force-include:j_one:n_b": old("Because"),
      "answer-anyway:j_one:n_c": old("Reviewed"),
      "snooze:j_two:n_a": old(SNOOZE, 5, 2),
      "done-evidence:j_one:n_d": old({ artifact: "https://example.test", note: "Done" }),
    });
    migrate(storage);
    expect(keysOf(storage)).toEqual(["j_one:n_a", "j_one:n_b", "j_one:n_c", "j_one:n_d", "j_two:n_a"].map((each) => `${P}actions:${each}`));
    expect(read(storage, "actions:j_one:n_a")).toMatchObject({ case: "keep", value: "Needed" });
    expect(read(storage, "actions:j_one:n_b")).toMatchObject({ case: "include", value: "Because" });
    expect(read(storage, "actions:j_one:n_c")).toMatchObject({ case: "anyway", value: "Reviewed" });
    expect(read(storage, "actions:j_two:n_a")).toMatchObject({ case: "snooze", seen: { base: 5, deployment: 2 } });
    expect(read(storage, "actions:j_one:n_d")).toMatchObject({ case: "evidence", value: { artifact: "https://example.test", note: "Done" } });
  });

  it("turns evidence kept as the artifact address alone into the artifact and an empty note", () => {
    const storage = memoryStorage();
    drafts(storage, { "done-evidence:j_one:n_a": old("https://example.test") });
    migrate(storage);
    expect(read(storage, "actions:j_one:n_a")).toMatchObject({ case: "evidence", value: { artifact: "https://example.test", note: "" } });
  });

  it("moves a reason typed under a rejection, which shared the bypass key, to its own", () => {
    const storage = memoryStorage();
    drafts(storage, { "bypass:j_one:n_a": "Reviewed out of band", "bypass:j_one:n_b": old("Form reason") });
    migrate(storage);
    expect(read(storage, "rejected-bypass:j_one:n_a")).toBe("Reviewed out of band");
    expect(read(storage, "actions:j_one:n_b")).toMatchObject({ case: "bypass", value: "Form reason" });
    expect(storage.getItem(`${P}bypass:j_one:n_a`)).toBeNull();
  });

});

describe("an old per-form draft that is not simply converted", () => {
  it("leaves a draft the new build already made, and removes the old ones beside it", () => {
    const storage = memoryStorage();
    drafts(storage, { "actions:j_one:n_a": opening("keep", "New", { base: 4, deployment: 1 }), "skip:j_one:n_a": old("Old") });
    migrate(storage);
    expect(read(storage, "actions:j_one:n_a")).toMatchObject({ case: "keep", value: "New" });
    expect(keysOf(storage)).toEqual([`${P}actions:j_one:n_a`]);
  });

  it("removes what cannot be read, and what names no journey", () => {
    const storage = memoryStorage();
    drafts(storage, { "skip:j_one:n_a": "RAW:{not json", "keep:j_one:n_b": old(7), "skip:not-a-journey:n_a": old("Why"), "snooze:j_one:n_c": old({ ...SNOOZE, until: "never" }) });
    migrate(storage);
    expect(keysOf(storage)).toEqual([]);
  });

  it("does nothing a second time", () => {
    const storage = memoryStorage();
    drafts(storage, { "skip:j_one:n_a": old("Why") });
    migrate(storage);
    const once = keysOf(storage).map((key) => [key, storage.getItem(key)]);
    migrate(storage);
    expect(keysOf(storage).map((key) => [key, storage.getItem(key)])).toEqual(once);
  });

  it("keeps every old key when the new draft cannot be stored", () => {
    const storage = memoryStorage();
    drafts(storage, { "skip:j_one:n_a": old("Why"), "bypass:j_one:n_b": "Reviewed" });
    const refusing = Object.assign(storage, {
      setItem: () => {
        throw new DOMException("full", "QuotaExceededError");
      },
    });
    migrate(refusing);
    expect(keysOf(storage)).toEqual([`${P}bypass:j_one:n_b`, `${P}skip:j_one:n_a`]);
  });

  it("does nothing where session storage is unavailable", () => {
    expect(() => { migrateTabDrafts(undefined, tabEditStore(() => undefined)); }).not.toThrow();
  });
});

describe("an old rejected draft", () => {
  const rejection = { rejection: "invalid", violations: [{ at: {}, code: "limit_exceeded", message: "Too long (F5)" }] };
  const attempt = (mutations: unknown[]) => ({ attempt: { mutations, base: 3, deployment: 1 }, rejection, address: "/journeys/j_one/n_a" });
  const skip = { op: "transition", node: "n_a", transition: "skip", reason: "Why" };

  it("becomes a rejected write the edit manager restores, with the attempt's patch and where it happened", async () => {
    const storage = memoryStorage();
    drafts(storage, { "rejected:j_one:dates:n_a": attempt([skip]) });
    migrate(storage);
    expect(keysOf(storage).filter((key) => key.startsWith(P))).toEqual([]);
    const { writes } = await startRoot({ storage });
    const views = Object.values(writes()).map((entry) => writeView(entry.snapshot));
    expect(views).toHaveLength(1);
    expect(views[0]).toMatchObject({ state: "rejected", about: { edit: "node:j_one:dates:n_a", address: "/journeys/j_one/n_a" }, command: { patch: { base_revision: 3, mutations: [skip] } } });
  });

  it("belongs to the actions when it was the snooze's", async () => {
    const storage = memoryStorage();
    drafts(storage, { "rejected:j_one:snooze:n_a": attempt([skip]) });
    migrate(storage);
    const { writes } = await startRoot({ storage });
    expect(Object.values(writes()).map((entry) => entry.snapshot.context.about.edit)).toEqual(["node:j_one:actions:n_a"]);
  });

  it("names the deployment revision only for a patch that writes an entity, as it was sent", () => {
    const storage = memoryStorage();
    const assign = { op: "set_participation", node: "n_a", kind: "k_owner", source: ["e_one"] };
    drafts(storage, { "rejected:j_one:assign:n_a": attempt([assign]), "rejected:j_one:dates:n_a": attempt([skip]) });
    migrate(storage);
    const kept = tabEditStore(() => storage).read().records;
    const byEdit = Object.fromEntries(kept.map((record) => [record.about.edit, record.command.patch.deployment_revision]));
    expect(byEdit).toEqual({ "node:j_one:assign:n_a": 1, "node:j_one:dates:n_a": undefined });
  });

  it("keeps the draft when the record cannot be stored, and removes one that cannot be read", () => {
    const storage = memoryStorage();
    drafts(storage, { "rejected:j_one:dates:n_a": attempt([skip]), "rejected:j_one:dates:n_b": { attempt: 3 } });
    const refusing: EditStore = { read: () => ({ records: [], dropped: 0 }), keep: () => false, forget: () => undefined };
    migrate(storage, refusing);
    expect(keysOf(storage)).toEqual([`${P}rejected:j_one:dates:n_a`]);
  });
});
