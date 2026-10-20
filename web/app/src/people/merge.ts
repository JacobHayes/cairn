// E6: merging one identity into another, as a workflow over the edit manager's writes: every
// journey referring to either (any status, archived among them) is read first, then the
// deployment patch names each at the revision seen, so the host checks each as it would derive
// after the merge and rechecks the set when it commits.
//
//   idle -> reading -> delivering -> idle        merge { survivor, merged, deployment }
//
// The two working states are `busy` and take no new start. The write is the manager's, found
// by the flow's edit key: stopping the flow (the control going away) stops waiting for the
// answer and never the write.
import { assign, enqueueActions, fromPromise, setup, type ActorRefFrom } from "xstate";

import { aboutOf, deliverWrite, type EditsRef } from "../data/edit-manager.ts";
import type { Answer, Command } from "../data/edit-record.ts";
import type { JourneySummary } from "../data/reads.ts";
import { currentAddress } from "../data/session.ts";
import { newPatchId, patchOf } from "../data/writes.ts";
import { mergeMutation } from "./model.ts";

/** What the flow is started with: the manager, its write's edit key, and how to find the journeys referring to the identities. */
export interface MergeInput {
  edits: EditsRef;
  key: string;
  referencing: (identities: readonly string[]) => Promise<JourneySummary[]>;
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
  /** The journeys read, to be named in the patch. */
  journeys: JourneySummary[];
  /** The patch id of the write now being delivered. */
  id: string;
  /** Why the journeys could not be read, until the next start. */
  failure: string | undefined;
}

type MergeEvent = { type: "merge" } & Merge;

/** What the flow tells whoever started it: the merge landed. */
export interface Merged {
  type: "merged";
  answer: Answer;
}

/** E6: the deployment patch merging `merged` into `survivor`, naming `referencing` at the revisions seen. */
export function mergeCommand(merge: Merge, referencing: readonly JourneySummary[], id: string = newPatchId()): Command {
  return {
    kind: "patch",
    patch: patchOf({ target: "deployment", baseRevision: merge.deployment, mutations: [mergeMutation(merge.survivor, merge.merged, referencing)] }, id),
  };
}

const readJourneys = fromPromise<JourneySummary[], { referencing: MergeInput["referencing"]; identities: string[] }>(({ input }) => input.referencing(input.identities));

export const mergeFlow = setup({
  types: { context: {} as MergeContext, events: {} as MergeEvent, input: {} as MergeInput, emitted: {} as Merged },
  actors: { readJourneys, deliver: deliverWrite },
  guards: {
    answered: (_, answer: Answer | undefined) => answer !== undefined,
  },
  actions: {
    mintId: assign({ id: () => newPatchId() }),
  },
}).createMachine({
  id: "merge",
  context: ({ input }) => ({ ...input, asked: undefined, journeys: [], id: "", failure: undefined }),
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
        src: "readJourneys",
        input: ({ context }) => ({ referencing: context.referencing, identities: [context.asked?.survivor ?? "", ...(context.asked?.merged ?? [])] }),
        onDone: { target: "delivering", actions: assign({ journeys: ({ event }) => event.output }) },
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
            sent: { id: context.id, command: mergeCommand(context.asked, context.journeys, context.id), about: aboutOf(context.key, undefined, currentAddress()) },
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
