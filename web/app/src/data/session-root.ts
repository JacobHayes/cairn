// ARCHITECTURE, Web UI: the session root. Started once at boot, outside React and StrictMode,
// it holds what the whole tab shares and its children: whether the tab is compatible with the
// server's build or in version skew (which absorbs the old latch), and the edit manager, which
// owns every write. The other owners (the subscription, the journeys, the activity) join as
// their steps land.
//
// Skew never lifts, so `skewed` is not final: a final root would stop its children, and the
// kept writes with them. In skew the tab derives, previews, writes and retries nothing (the
// old engine's touched sets may be wrong) and asks for a reload; unsent edits are records in
// session storage, so the reload keeps them.
import { sendTo, setup, type ActorRefFrom } from "xstate";

import { editManager, type EditsInput } from "./edit-manager.ts";
import type { WriteEnv } from "./write-machine.ts";

/** What the root is started with: the manager's input, less what the root itself answers. */
export interface RootInput {
  edits: Omit<EditsInput, "env"> & { env: Omit<WriteEnv, "compatible"> };
}

export const sessionRoot = setup({
  types: {
    context: {} as RootInput,
    events: {} as
      | { type: "skew" }
      /** The connection changed: whatever waits looks again. */
      | { type: "wake" }
      /** Puts the kept edits back, once the root is running. */
      | { type: "restore" },
    input: {} as RootInput,
  },
  actors: { edits: editManager },
}).createMachine({
  id: "session",
  context: ({ input }) => input,
  invoke: {
    id: "edits",
    src: "edits",
    input: ({ context, self }) => ({
      ...context.edits,
      env: { ...context.edits.env, compatible: () => self.getSnapshot().matches("compatible") },
    }),
  },
  initial: "compatible",
  states: {
    compatible: { on: { skew: "skewed" } },
    // Waiting writes give up at once: the wake is delivered after this transition commits.
    skewed: { entry: sendTo("edits", { type: "wake" }) },
  },
  on: {
    wake: { actions: sendTo("edits", { type: "wake" }) },
    restore: { actions: sendTo("edits", { type: "restore" }) },
  },
});

export type SessionRootRef = ActorRefFrom<typeof sessionRoot>;
export type EditsRef = ActorRefFrom<typeof editManager>;

/** The root's edit manager. */
export function editsOf(root: SessionRootRef): EditsRef {
  const edits = root.getSnapshot().children.edits;
  if (edits === undefined) {
    throw new Error("the session root is not running its edit manager");
  }
  return edits;
}

/** What the journeys are given of version skew: whether the tab is in it, and the way into it. */
export interface Skew {
  skewed(): boolean;
  latch(): void;
}
