// A session root over a fake host for the edit tests: the real root and manager on a simulated
// clock, a tab's storage in memory, and the notes the manager made.
import type { Patch } from "@cairn/client";
import { SimulatedClock, createActor } from "xstate";

import type { About } from "./edit-record.ts";
import { tabEditStore } from "./edit-record.ts";
import { FakeHost, memoryStorage, settled, writeEnv } from "./fake.test-support.ts";
import { editsOf } from "./session-root.ts";
import { sessionRoot } from "./session-root.ts";
import { patchOf } from "./writes.ts";

/** A patch renaming `node` of journey `j_one`, with the id given. */
export function renaming(id: string, node = "n_a", base = 3): Patch {
  return patchOf({ target: { journey: "j_one" }, baseRevision: base, mutations: [{ op: "set_node_field", node, value: { title: `Renamed ${node}` } }] }, id);
}

/** What the edit `node` of journey `j_one` is about. */
export function aboutNode(node = "n_a"): About {
  return { edit: `title:j_one:${node}`, label: `Step ${node}`, address: `/journeys/j_one/${node}` };
}

export interface StartOptions {
  host?: FakeHost;
  storage?: Storage;
  clock?: SimulatedClock;
  /** Whether to send `restore` after the root starts (the session does). */
  restore?: boolean;
}

/** A running root over a fake host; `edits` is its manager. */
export async function startRoot(options: StartOptions = {}) {
  const host = options.host ?? new FakeHost();
  const storage = options.storage ?? memoryStorage();
  const clock = options.clock ?? new SimulatedClock();
  const { parts, switches, saves } = writeEnv(host);
  const notes = { dropped: [] as number[], unstored: 0 };
  const root = createActor(sessionRoot, {
    clock,
    input: {
      edits: {
        store: tabEditStore(() => storage),
        env: parts,
        notes: {
          dropped: (count) => notes.dropped.push(count),
          unstored: () => {
            notes.unstored += 1;
          },
        },
      },
    },
  });
  root.start();
  const edits = editsOf(root);
  if (options.restore !== false) {
    root.send({ type: "restore" });
    await settled();
  }
  const submit = (patch: Patch, about: About = aboutNode()) => {
    edits.send({ type: "submit", id: patch.id, command: { kind: "patch", patch }, about });
  };
  const advance = async (ms: number) => {
    clock.increment(ms);
    await settled();
  };
  return { host, storage, clock, switches, saves, notes, root, edits, submit, advance, writes: () => edits.getSnapshot().context.writes };
}
