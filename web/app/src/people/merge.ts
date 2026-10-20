// E6: merging several identities into one, as a workflow over the edit manager's writes: what
// refers to each of them (the references read, any journey status, archived among them) is read
// first, then the deployment patch names each journey at the revision seen, so the host checks
// each as it would derive after the merge and rechecks the set when it commits.
//
//   idle -> reading -> delivering -> idle        merge { survivor, merged, deployment }
//
// The two working states are `busy` and take no new start. The write is the manager's, found
// by the flow's edit key: stopping the flow (the control going away) stops waiting for the
// answer and never the write.
import { assign, enqueueActions, fromPromise, setup, type ActorRefFrom } from "xstate";

import { aboutOf, deliverWrite, type EditsRef } from "../data/edit-manager.ts";
import type { Answer, Command } from "../data/edit-record.ts";
import type { IdentityReferences } from "../data/host.ts";
import { currentAddress } from "../data/session.ts";
import { newPatchId, patchOf } from "../data/writes.ts";
import { mergeMutation } from "./model.ts";

/** What the flow is started with: the manager, its write's edit key, and the references read of one identity. */
export interface MergeInput {
  edits: EditsRef;
  key: string;
  references: (identity: string) => Promise<IdentityReferences>;
}

/** What a merge asks for. */
export interface Merge {
  survivor: string;
  merged: readonly string[];
  /** The deployment revision the merge is drafted against. */
  deployment: number;
}

interface MergeContext extends MergeInput {
  asked: Merge | undefined;
  /** What refers to each of them, as read, for the patch to name. */
  read: IdentityReferences[];
  /** The patch id of the write now being delivered. */
  id: string;
  /** Why the references could not be read, until the next start. */
  failure: string | undefined;
}

type MergeEvent = { type: "merge" } & Merge;

/** What the flow tells whoever started it: the merge landed. */
export interface Merged {
  type: "merged";
  answer: Answer;
}

/** E6: the deployment patch merging `merged` into `survivor`, naming the journeys `referencing` lists at the revisions seen. */
export function mergeCommand(merge: Merge, referencing: readonly IdentityReferences[], id: string = newPatchId()): Command {
  return {
    kind: "patch",
    patch: patchOf({ target: "deployment", baseRevision: merge.deployment, mutations: [mergeMutation(merge.survivor, merge.merged, referencing)] }, id),
  };
}

const readReferences = fromPromise<IdentityReferences[], { references: MergeInput["references"]; identities: string[] }>(({ input }) => Promise.all(input.identities.map(input.references)));

export const mergeFlow = setup({
  types: { context: {} as MergeContext, events: {} as MergeEvent, input: {} as MergeInput, emitted: {} as Merged },
  actors: { readReferences, deliver: deliverWrite },
  guards: {
    answered: (_, answer: Answer | undefined) => answer !== undefined,
  },
  actions: {
    mintId: assign({ id: () => newPatchId() }),
  },
}).createMachine({
  id: "merge",
  context: ({ input }) => ({ ...input, asked: undefined, read: [], id: "", failure: undefined }),
  initial: "idle",
  states: {
    idle: {
      on: {
        merge: {
          target: "reading",
          actions: assign({ asked: ({ event }) => ({ survivor: event.survivor, merged: event.merged, deployment: event.deployment }), failure: undefined }),
        },
      },
    },
    reading: {
      tags: ["busy"],
      invoke: {
        src: "readReferences",
        input: ({ context }) => ({ references: context.references, identities: [context.asked?.survivor ?? "", ...(context.asked?.merged ?? [])] }),
        onDone: { target: "delivering", actions: assign({ read: ({ event }) => event.output }) },
        onError: { target: "idle", actions: assign({ failure: ({ event }) => (event.error instanceof Error ? event.error.message : String(event.error)) }) },
      },
    },
    delivering: {
      tags: ["busy"],
      entry: "mintId",
      invoke: {
        src: "deliver",
        input: ({ context }) => {
          if (context.asked === undefined) {
            throw new Error("the merge has nothing asked");
          }
          return {
            edits: context.edits,
            sent: { id: context.id, command: mergeCommand(context.asked, context.read, context.id), about: aboutOf(context.key, undefined, currentAddress()) },
          };
        },
        onDone: {
          target: "idle",
          actions: enqueueActions(({ event, enqueue }) => {
            if (event.output !== undefined) {
              enqueue.emit({ type: "merged", answer: event.output });
            }
          }),
        },
      },
    },
  },
});

export type MergeRef = ActorRefFrom<typeof mergeFlow>;
