// A13, A11: importing a route file, as a workflow over the edit manager's writes. A person
// chooses a file; the flow reads it, parses it, and delivers it as an import write. Where
// another draft is open, the import is refused (A11) and the person may discard that draft and
// import: the flow reads the route, discards the draft as a write of its own, and delivers the
// import once that lands.
//
//   idle -> reading -> delivering -> idle        choose { file }
//   idle -> route -> delivering -> idle          replace { file }: no draft is open
//                -> discarding -> delivering     replace { file }: discard the open draft first
//
// Every step that works is `busy`, and a busy flow takes no new start. The writes are the
// manager's, found by the flow's edit keys: stopping the flow (the control going away) stops
// waiting for an answer and never the write. `route` is how a flow recovers when a discard
// landed after the flow stopped waiting (a Retry by hand, a reload, another tab): it reads the
// route as it is now, and skips the discard when no draft is open.
import { assign, enqueueActions, fromPromise, setup, type ActorRefFrom } from "xstate";

import { aboutOf, deliverWrite, submitWrite, type EditsRef, type Sent } from "../data/edit-manager.ts";
import type { Answer, Command } from "../data/edit-record.ts";
import type { Route, RouteFile } from "../data/host.ts";
import { currentAddress } from "../data/session.ts";
import { newPatchId, patchOf } from "../data/writes.ts";

/** What the flow is started with: the manager, the edit keys of its two writes, and how to read a file and a route. */
export interface ImportInput {
  edits: EditsRef;
  keys: { import: string; discard: string };
  /** A13: a file's text as the file document (YAML on disk, JSON on the wire); throws when it is not one. */
  read: (text: string) => RouteFile;
  /** A route with its draft (A11). */
  route: (id: string) => Promise<Route>;
}

interface ImportContext extends ImportInput {
  /** The file chosen, until it has been read. */
  chosen: File | undefined;
  /** The file read, and what to do with it. */
  file: RouteFile | undefined;
  /** The route's revision the discard is drafted against. */
  revision: number;
  /** The patch id of the write now being delivered. */
  id: string;
  /** Why the file could not be read or the route found, until the next start. */
  unreadable: string | undefined;
}

type ImportEvent =
  /** A file the person picked. */
  | { type: "choose"; file: File }
  /** A file already read, to import over the route's open draft. */
  | { type: "replace"; file: RouteFile };

/** What the flow tells whoever started it: the file landed as a draft. */
export interface Imported {
  type: "imported";
  file: RouteFile;
  answer: Answer;
}

/** A11: the discard of a route's open draft, drafted against the revision the route was read at. */
export function discardCommand(route: string, revision: number, id: string = newPatchId()): Command {
  return { kind: "patch", patch: patchOf({ target: { route }, baseRevision: revision, mutations: [{ op: "discard_draft" }] }, id) };
}

/** A13: the import of `file`, under the patch id `id`. */
export function importCommand(file: RouteFile, id: string = newPatchId()): Command {
  return { kind: "import", request: { patch_id: id, file } };
}

const messageOf = (thrown: unknown): string => (thrown instanceof Error ? thrown.message : String(thrown));

const readFile = fromPromise<RouteFile, { chosen: File; read: ImportInput["read"] }>(async ({ input }) => input.read(await input.chosen.text()));
const readRoute = fromPromise<Route, { id: string; route: ImportInput["route"] }>(({ input }) => input.route(input.id));
/** The import write, delivered: the file it was for, and its answer once it landed. */
const deliverImport = fromPromise<{ file: RouteFile; answer: Answer | undefined }, { edits: EditsRef; sent: Sent; file: RouteFile }>(async ({ input, signal }) => ({
  file: input.file,
  answer: await submitWrite(input.edits, input.sent, signal),
}));

export const importFlow = setup({
  types: { context: {} as ImportContext, events: {} as ImportEvent, input: {} as ImportInput, emitted: {} as Imported },
  actors: { readFile, readRoute, deliver: deliverWrite, deliverImport },
  guards: {
    answered: (_, answer: Answer | undefined) => answer !== undefined,
    noDraft: (_, route: Route) => route.draft == null,
  },
  actions: {
    mintId: assign({ id: () => newPatchId() }),
  },
}).createMachine({
  id: "import",
  context: ({ input }) => ({ ...input, chosen: undefined, file: undefined, revision: 0, id: "", unreadable: undefined }),
  initial: "idle",
  states: {
    idle: {
      on: {
        choose: { target: "reading", actions: assign({ chosen: ({ event }) => event.file, unreadable: undefined }) },
        replace: { target: "route", actions: assign({ file: ({ event }) => event.file, unreadable: undefined }) },
      },
    },
    reading: {
      tags: ["busy"],
      invoke: {
        src: "readFile",
        input: ({ context }) => ({ chosen: context.chosen ?? new File([], ""), read: context.read }),
        onDone: { target: "delivering", actions: assign({ file: ({ event }) => event.output, chosen: undefined }) },
        onError: { target: "idle", actions: assign({ unreadable: ({ event }) => `The file could not be imported: ${messageOf(event.error)}`, chosen: undefined }) },
      },
    },
    route: {
      tags: ["busy"],
      invoke: {
        src: "readRoute",
        input: ({ context }) => ({ id: context.file?.route ?? "", route: context.route }),
        onDone: [
          { guard: { type: "noDraft", params: ({ event }) => event.output }, target: "delivering" },
          { target: "discarding", actions: assign({ revision: ({ event }) => event.output.revision }) },
        ],
        onError: { target: "idle", actions: assign({ unreadable: ({ event }) => `The route could not be read: ${messageOf(event.error)}` }) },
      },
    },
    discarding: {
      tags: ["busy"],
      entry: "mintId",
      invoke: {
        src: "deliver",
        input: ({ context }) => ({
          edits: context.edits,
          sent: {
            id: context.id,
            command: discardCommand(context.file?.route ?? "", context.revision, context.id),
            about: aboutOf(context.keys.discard, context.file?.name, currentAddress()),
          },
        }),
        // A discard that did not land (rejected, or waiting for Retry) leaves the flow where it started.
        onDone: [{ guard: { type: "answered", params: ({ event }) => event.output }, target: "delivering" }, { target: "idle" }],
      },
    },
    delivering: {
      tags: ["busy"],
      entry: "mintId",
      invoke: {
        src: "deliverImport",
        input: ({ context }) => {
          if (context.file === undefined) {
            throw new Error("the import has no file to deliver");
          }
          return {
            edits: context.edits,
            file: context.file,
            sent: { id: context.id, command: importCommand(context.file, context.id), about: aboutOf(context.keys.import, context.file.name, currentAddress()) },
          };
        },
        onDone: {
          target: "idle",
          // Told only of a landing, which a person who left the screen is not around to hear.
          actions: enqueueActions(({ event, enqueue }) => {
            if (event.output.answer !== undefined) {
              enqueue.emit({ type: "imported", file: event.output.file, answer: event.output.answer });
            }
          }),
        },
      },
    },
  },
});

export type ImportRef = ActorRefFrom<typeof importFlow>;
