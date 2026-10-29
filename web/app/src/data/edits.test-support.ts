// A session root over a fake host for the edit tests: the real root and manager on a simulated
// clock.
import { streamStatus, type Patch } from "@cairn/client";
import { SimulatedClock, createActor } from "xstate";

import type { About } from "./edit-command.ts";
import { FakeDeriver, FakeHost, settled, writeEnv } from "./fake.test-support.ts";
import { editsOf, landedEvents, sessionRoot, subscriptionOf, syncOf, type SessionRootRef } from "./session-root.ts";
import { inFlightOf, problemsOf } from "./edit-manager.ts";
import { syncInputOf, syncSummary, type SyncReading, type SyncSummary } from "./sync.ts";
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
  clock?: SimulatedClock;
}

/** A running root over a fake host; `edits` is its manager. */
export async function startRoot(options: StartOptions = {}) {
  const host = options.host ?? new FakeHost();
  const clock = options.clock ?? new SimulatedClock();
  const { parts, switches, saves } = writeEnv(host);
  const root = createActor(sessionRoot, {
    clock,
    input: {
      host,
      deriver: new FakeDeriver(),
      open: host.openTicks,
      today: () => new Date("2026-10-06T12:00:00Z"),
      now: () => clock.now(),
      conversations: { assistant: host.assistant },
      edits: {
        // A landed write is logged, and told to the root as the session does.
        env: {
          ...parts,
          saved: (command, answer, began) => {
            parts.saved(command, answer, began);
            for (const event of landedEvents(command, answer, began, (_journey, node) => node, new Date("2026-10-06T14:02:11Z"))) {
              root.send(event);
            }
          },
        },
      },
    },
  });
  root.start();
  const edits = editsOf(root);
  await settled();
  const submit = (patch: Patch, about: About = aboutNode()) => {
    edits.send({ type: "submit", id: patch.id, command: { kind: "patch", patch }, about });
  };
  const advance = async (ms: number) => {
    clock.increment(ms);
    await settled();
  };
  return { host, clock, switches, saves, root, edits, submit, advance, writes: () => edits.getSnapshot().context.writes };
}

/**
 * What the sync chip says of a running root, as `useSync` reads it: the scheduler's facts and
 * its other owners', put through the rule. `read` overrides what a test wants held (the
 * network, the revision, the demo host).
 */
export function chipOf(root: SessionRootRef, read: Partial<SyncReading> = {}): SyncSummary {
  const snapshot = root.getSnapshot();
  const writes = editsOf(root).getSnapshot().context.writes;
  return syncSummary(
    syncInputOf(syncOf(root).getSnapshot().context.timing, {
      stream: streamStatus(subscriptionOf(root).getSnapshot()),
      online: true,
      skew: snapshot.matches("skewed"),
      revision: undefined,
      count: inFlightOf(writes),
      problems: problemsOf(writes, snapshot.matches("skewed")),
      latest: snapshot.context.recent.saves[0],
      ...read,
    }),
  );
}
