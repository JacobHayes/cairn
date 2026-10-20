// H5, H6: every write path (a section, authoring, a screen, a proposal review, drafting a
// proposal and opening it, an insertion, an import, a merge) hands the edit manager a command
// under a stable edit key, so a refusal of any of them is held, listed once on the chip, kept
// across a reload with the command that was refused, and dropped by Discard or by the edit's
// next write, one way for all of them.
import type { Answered, HttpFailure, Rejection } from "@cairn/client";
import { describe, expect, it } from "vitest";

import { authorCommand, authorEdit } from "../authoring/write.ts";
import type { Authored } from "../authoring/target.ts";
import { nodeCommand, nodeEdit } from "../detail/write.ts";
import { mergeCommand } from "../people/merge.ts";
import { proposalCommand } from "../proposals/write.ts";
import { discardCommand, importCommand } from "../routes/import.ts";
import { screenCommand } from "../screens/write.ts";
import { aboutOf, problemsOf, submitWrite, type Sent } from "./edit-manager.ts";
import type { Command } from "./edit-record.ts";
import { startRoot } from "./edits.test-support.ts";
import { FakeHost, settled } from "./fake.test-support.ts";
import type { RouteFile } from "./host.ts";
import type { WriteSnapshot } from "./write-machine.ts";
import { idOf } from "./edit-record.ts";

const invalid: Rejection = { rejection: "invalid", violations: [{ at: {}, code: "limit_exceeded", message: "Too long (F5)" }] };
const refusal: Answered<HttpFailure> = { outcome: "rejected", rejection: invalid };
const rejected = { outcome: "rejected" as const, rejection: invalid };
const file = { format: 1, route: "r_one", name: "One", nodes: [] } as unknown as RouteFile;
const authored = { target: { journey: "j_one" } } as Authored;
const rename = [{ op: "set_node_field" as const, node: "n_a", value: { title: "Renamed" } }];

interface Path {
  name: string;
  edit: string;
  command: (id: string) => Command;
  /** Makes the host refuse the write. */
  refuse: (host: FakeHost) => void;
}

const patchRefused = (host: FakeHost) => {
  host.answers = [refusal, refusal, refusal];
};

const paths: Path[] = [
  { name: "a section's edit", edit: nodeEdit("j_one", "title:n_a"), command: (id) => nodeCommand("j_one", rename, { base: 3, deployment: 1 }, id), refuse: patchRefused },
  { name: "an authoring edit", edit: authorEdit(authored, "local-edits:n_a"), command: (id) => authorCommand(authored, rename, 3, id), refuse: patchRefused },
  {
    name: "a screen's write",
    edit: "journey-header:j_one",
    command: (id) => screenCommand({ target: { journey: "j_one" }, baseRevision: 3, mutations: rename }, id),
    refuse: patchRefused,
  },
  {
    name: "a proposal review",
    edit: "proposal:pr_one",
    command: (id) => proposalCommand({ op: "apply", destination: { journey: "j_one" }, proposal: "pr_one", request: { reviewed_revision: 4 } }, id),
    refuse: (host) => {
      host.proposed = [rejected, rejected, rejected];
    },
  },
  {
    name: "drafting a proposal and opening it",
    edit: "upgrade:j_one",
    command: (id) => proposalCommand({ op: "upgrade", journey: "j_one", request: { proposal: "pr_new", to: 2 } }, id),
    refuse: (host) => {
      host.proposed = [rejected, rejected, rejected];
    },
  },
  {
    name: "an insertion's review",
    edit: "insert:journey:j_one",
    command: (id) => proposalCommand({ op: "create", destination: { journey: "j_one" }, request: { id: "pr_new", draft: { base_revision: 3 } as never } }, id),
    refuse: (host) => {
      host.proposed = [rejected, rejected, rejected];
    },
  },
  {
    name: "an import",
    edit: "import:new-route",
    command: (id) => importCommand(file, id),
    refuse: (host) => {
      host.imports = [refusal, refusal, refusal];
    },
  },
  {
    name: "a merge",
    edit: "merge-identities",
    command: (id) => mergeCommand({ survivor: "who_keep", merged: "who_old", deployment: 7 }, [{ id: "j_one", revision: 4 } as never], id),
    refuse: patchRefused,
  },
  {
    name: "an import's discard",
    edit: "import:new-route:discard",
    command: (id) => discardCommand("r_one", 5, id),
    refuse: patchRefused,
  },
];

const sentOf = (path: Path, id: string): Sent => {
  const command = path.command(id);
  return { id: idOf(command), command, about: aboutOf(path.edit, "Draft the plan", `/at/${path.edit}`) };
};

describe.each(paths)("$name", (path) => {
  it("is held when refused, listed once on the chip with its label and address, and keeps the command refused", async () => {
    const { host, edits, writes, storage } = await startRoot();
    path.refuse(host);
    const sent = sentOf(path, "p_a");
    expect(await submitWrite(edits, sent)).toBeUndefined();
    expect(problemsOf(writes(), false)).toEqual([{ id: "p_a", kind: "rejected", message: expect.any(String) as string, label: "Draft the plan", address: `/at/${path.edit}`, retry: false }]);
    expect((writes().p_a?.snapshot as WriteSnapshot).context.command).toEqual(sent.command);
    expect(storage.length).toBe(1);
  });

  it("is still there, with the same command, in a new root over the same storage, and nothing is sent again", async () => {
    const first = await startRoot();
    path.refuse(first.host);
    const sent = sentOf(path, "p_a");
    await submitWrite(first.edits, sent);
    first.root.stop();
    const second = await startRoot({ storage: first.storage });
    expect(problemsOf(second.writes(), false).map((problem) => [problem.id, problem.kind])).toEqual([["p_a", "rejected"]]);
    expect((second.writes().p_a?.snapshot as WriteSnapshot).context.command).toEqual(sent.command);
    expect([second.host.sent, second.host.called, second.host.imported]).toEqual([[], [], []]);
  });

  it("is dropped, record and all, by Discard", async () => {
    const { host, edits, writes, storage } = await startRoot();
    path.refuse(host);
    await submitWrite(edits, sentOf(path, "p_a"));
    edits.send({ type: "discard", id: "p_a" });
    await settled();
    expect(writes()).toEqual({});
    expect(storage.length).toBe(0);
  });

  it("gives way to the edit's next write, which is sent as itself, and not to a write of another edit", async () => {
    const { host, edits, writes } = await startRoot();
    path.refuse(host);
    await submitWrite(edits, sentOf(path, "p_a"));
    const other: Sent = { ...sentOf(path, "p_other"), about: aboutOf(`${path.edit}:elsewhere`, undefined, undefined) };
    await submitWrite(edits, other);
    expect(Object.keys(writes()).sort()).toEqual(["p_a", "p_other"]);
    await submitWrite(edits, sentOf(path, "p_b"));
    expect(Object.keys(writes()).sort()).toEqual(["p_b", "p_other"]);
  });
});

describe("a stale write that cannot be rebased", () => {
  it("is rejected once for a proposal, even where a patch would be rebased on the new revision, and is sent no more", async () => {
    const host = new FakeHost();
    host.overlaps = () => false;
    const { edits, writes } = await startRoot({ host });
    host.proposed = [{ outcome: "rejected", rejection: { rejection: "stale", conflicts: [{ of: { proposal: "pr_one" }, expected: 4, current: 5 }], intervening: [] } }];
    await submitWrite(edits, sentOf(paths[3] as Path, "p_a"));
    expect(host.called).toEqual(["apply"]);
    expect(problemsOf(writes(), false).map((problem) => problem.kind)).toEqual(["conflict"]);
  });
});

describe("the edit keys", () => {
  it("name only the control and what it is about, so a control that is shown again finds its write", () => {
    expect(nodeEdit("j_one", "title:n_a")).toBe(nodeEdit("j_one", "title:n_a"));
    expect(nodeEdit("j_one", "title:n_a")).not.toBe(nodeEdit("j_two", "title:n_a"));
    expect(nodeEdit("j_one", "title:n_a")).not.toBe(nodeEdit("j_one", "title:n_b"));
    expect(authorEdit(authored, "edges:n_a")).toBe(authorEdit({ target: { journey: "j_one" } } as Authored, "edges:n_a"));
    expect(authorEdit(authored, "edges:n_a")).not.toBe(authorEdit({ target: { route: "j_one" } } as Authored, "edges:n_a"));
  });
});
