// A3, H5: drawing an edge on the canvas, as a machine the page owns. An author starts from a
// node, then picks the node it requires:
//
//   inactive -> choosing -> submitting -> inactive
//
// `choosing` takes a pick: the engine's preview refuses an edge it cannot take, and the pick stays
// in `choosing` with its words; any other is previewed (unless the tab is in version skew, which previews nothing) and handed to the
// edit manager as a write. From then on the write is the manager's: the drawing only watches it
// by its id. Stopping the drawing (the button, Escape, leaving edit mode) stops the drawing
// waiting for the answer and never touches the write, which goes on to land or to ask the person
// on the sync chip. A write that lands closes the drawing by the same reset. One that needs the
// person leaves the drawing choosing, still watching it: a Retry or a Discard from the banner or
// from the chip moves the drawing the same way. Only the watcher of the write the drawing last
// sent can close it, so an earlier write's answer can never close a later drawing. While
// submitting, a pick or a start is not heard at all.
import { assign, fromCallback, fromPromise, setup, type SnapshotFrom } from "xstate";

import type { EditsRef } from "../data/session-root.ts";
import type { About, OperationId, PatchCommand } from "../data/edit-command.ts";
import type { Rejection } from "../data/writes.ts";
import { titleIn, type Mutation } from "./graph.ts";
import type { Authored } from "./target.ts";
import type { Preview } from "./write.ts";
import { authoredPatchOf } from "./write.ts";

/** Where Escape is heard while drawing: `listen` starts hearing and answers how to stop. */
export interface KeySource {
  listen(cancel: () => void): () => void;
}

export interface DrawingInput {
  edits: EditsRef;
  /** The engine's answer to `mutations` on `authored`, as `user`. */
  preview: (authored: Authored, mutations: Mutation[], user: string) => Promise<Preview>;
  /** Whether the tab is not in version skew. */
  compatible: () => boolean;
  keys: KeySource;
}

/** Why a pick sent nothing. */
export type Refusal =
  /** The engine refused the edge locally. */
  | { by: "preview"; rejection: Rejection }
  /** This edge's earlier write is still out. */
  | { by: "busy" };

/** A pick on its way: what the preview and the submit need. Dropped once submitted. */
interface Picked {
  to: string;
  id: OperationId;
  command: PatchCommand;
  about: About;
  authored: Authored;
  user: string;
}

/** A pick handed to the edit manager: what the drawing watches and shows. The manager sends the command as it is; a pick is not kept. */
export interface Sent {
  to: string;
  id: OperationId;
}

interface DrawingContext extends DrawingInput {
  from: string | undefined;
  refused: Refusal | undefined;
  /** Set only while previewing (and the instant before a submit). */
  picked: Picked | undefined;
  /** Set while the write is out or needs the person. */
  sent: Sent | undefined;
}

export type DrawingEvent =
  | { type: "start"; from: string }
  | { type: "pick"; to: string; authored: Authored; user: string; address: string | undefined }
  | { type: "cancel" }
  /** The write the drawing sent landed. */
  | { type: "write.landed" }
  /** It needs the person: rejected, stopped, gone, or out of resends. */
  | { type: "write.unlanded" }
  /** It is out, or being resent (after a Retry too). */
  | { type: "write.busy" }
  /** It ended without landing: it was discarded. */
  | { type: "write.ended" }
  /** The manager holds no write under the id: it refused the submit. */
  | { type: "write.missing" };

/** The edit an edge is: one edge is one edit wherever it is drawn, so the manager keeps at most one write of it busy. */
export function edgeEdit(authored: Pick<Authored, "target">, from: string, to: string): string {
  const domain = "journey" in authored.target ? `journey:${authored.target.journey}` : `route:${authored.target.route}`;
  return `edge:${domain}:${from}:${to}`;
}

const pickedOf = (context: DrawingContext): Picked => {
  if (context.picked === undefined) {
    throw new Error("no pick is on its way");
  }
  return context.picked;
};

const sentOf = (context: DrawingContext): Sent => {
  if (context.sent === undefined) {
    throw new Error("no write was sent");
  }
  return context.sent;
};

const pickOf = (event: DrawingEvent): Extract<DrawingEvent, { type: "pick" }> => {
  if (event.type !== "pick") {
    throw new Error(`expected a pick, got ${event.type}`);
  }
  return event;
};

const startOf = (event: DrawingEvent): Extract<DrawingEvent, { type: "start" }> => {
  if (event.type !== "start") {
    throw new Error(`expected a start, got ${event.type}`);
  }
  return event;
};

const previewedOf = ({ event }: { event: { output: Preview } }): Preview => event.output;

const FORGOTTEN = { from: undefined, refused: undefined, picked: undefined, sent: undefined } as const;

export const edgeDrawing = setup({
  types: { context: {} as DrawingContext, events: {} as DrawingEvent, input: {} as DrawingInput },
  actors: {
    preview: fromPromise<Preview, { picked: Picked; preview: DrawingInput["preview"] }>(({ input }) => {
      const { authored, command, user } = input.picked;
      return input.preview(authored, command.patch.mutations, user);
    }),
    // Watches the write by its id, from its current snapshot on (a write stopped at once in skew
    // is already waiting for the person). A stopped watcher is never heard again.
    observe: fromCallback<DrawingEvent, { edits: EditsRef; id: OperationId }>(({ input, sendBack }) => {
      const entry = input.edits.getSnapshot().context.writes[input.id];
      if (entry === undefined) {
        sendBack({ type: "write.missing" });
        return undefined;
      }
      const look = (snapshot: typeof entry.snapshot) => {
        if (snapshot.status === "done") {
          sendBack({ type: snapshot.matches("landed") ? "write.landed" : "write.ended" });
        } else if (snapshot.hasTag("needsYou")) {
          sendBack({ type: "write.unlanded" });
        } else if (snapshot.hasTag("busy")) {
          sendBack({ type: "write.busy" });
        }
      };
      look(entry.ref.getSnapshot());
      const subscription = entry.ref.subscribe(look);
      return () => {
        subscription.unsubscribe();
      };
    }),
    keys: fromCallback<DrawingEvent, KeySource>(({ input, sendBack }) =>
      input.listen(() => {
        sendBack({ type: "cancel" });
      }),
    ),
  },
  guards: {
    incompatible: ({ context }) => !context.compatible(),
    previewRefused: (_, preview: Preview) => preview.outcome === "rejected",
  },
  actions: {
    forget: assign(FORGOTTEN),
    begin: assign(({ event }) => ({ ...FORGOTTEN, from: startOf(event).from })),
    take: assign(({ context, event }) => {
      const { authored, to, user, address } = pickOf(event);
      const from = context.from ?? "";
      const patch = authoredPatchOf(authored, [{ op: "add_edge", edge: { node: from, requires: to } }]);
      const about: About = { edit: edgeEdit(authored, from, to), label: titleIn(authored.tree, from), ...(address === undefined ? {} : { address }) };
      return { refused: undefined, sent: undefined, picked: { to, id: patch.id, command: { kind: "patch" as const, patch }, about, authored, user } };
    }),
    submit: ({ context }) => {
      const { id, command, about } = pickedOf(context);
      context.edits.send({ type: "submit", id, command, about });
    },
    handOver: assign(({ context }) => {
      const { to, id } = pickedOf(context);
      return { sent: { to, id }, picked: undefined };
    }),
    refusePreview: assign((_, preview: Preview) => ({ refused: preview.outcome === "rejected" ? { by: "preview" as const, rejection: preview.rejection } : undefined, picked: undefined })),
    refuseBusy: assign({ refused: { by: "busy" as const }, sent: undefined }),
    dropSent: assign({ sent: undefined }),
  },
}).createMachine({
  id: "edgeDrawing",
  context: ({ input }) => ({ ...input, ...FORGOTTEN }),
  initial: "inactive",
  states: {
    // Every way out of drawing ends here, and this entry is the one reset.
    inactive: { entry: "forget", on: { start: { target: "active", actions: "begin" } } },
    active: {
      invoke: { src: "keys", input: ({ context }) => context.keys },
      on: { cancel: "inactive" },
      initial: "choosing",
      states: {
        choosing: {
          initial: "fresh",
          on: {
            start: { target: ".fresh", actions: "begin" },
            pick: [
              { guard: "incompatible", target: "submitting.awaiting", actions: ["take", "submit", "handOver"] },
              { target: "submitting", actions: "take" },
            ],
          },
          states: {
            fresh: {},
            // The write needs the person; the drawing still follows it, so a Retry or a Discard from anywhere moves it.
            unlanded: {
              invoke: { src: "observe", input: ({ context }) => ({ edits: context.edits, id: sentOf(context).id }) },
              on: {
                "write.busy": "#edgeDrawing.active.submitting.awaiting",
                "write.landed": "#edgeDrawing.inactive",
                "write.ended": { target: "fresh", actions: "dropSent" },
                "write.missing": { target: "fresh", actions: "dropSent" },
              },
            },
          },
        },
        submitting: {
          initial: "previewing",
          states: {
            previewing: {
              invoke: {
                id: "preview",
                src: "preview",
                input: ({ context }) => ({ picked: pickedOf(context), preview: context.preview }),
                onDone: [
                  { guard: { type: "previewRefused", params: previewedOf }, target: "#edgeDrawing.active.choosing.fresh", actions: { type: "refusePreview", params: previewedOf } },
                  { target: "awaiting", actions: ["submit", "handOver"] },
                ],
                // The engine could not say; the host will.
                onError: { target: "awaiting", actions: ["submit", "handOver"] },
              },
            },
            awaiting: {
              invoke: { src: "observe", input: ({ context }) => ({ edits: context.edits, id: sentOf(context).id }) },
              on: {
                "write.landed": "#edgeDrawing.inactive",
                "write.unlanded": "#edgeDrawing.active.choosing.unlanded",
                "write.ended": { target: "#edgeDrawing.active.choosing.fresh", actions: "dropSent" },
                "write.missing": { target: "#edgeDrawing.active.choosing.fresh", actions: "refuseBusy" },
              },
            },
          },
        },
      },
    },
  },
});

export type DrawingSnapshot = SnapshotFrom<typeof edgeDrawing>;

/** What a component reads of the drawing; tagged, so none pieces it from optional context fields. */
export type DrawingView =
  | { state: "inactive" }
  /** Waiting for a pick; `sent` is the write that needs the person, if there is one. */
  | { state: "choosing"; from: string; refused: Refusal | undefined; sent: Sent | undefined }
  /** A pick being previewed; nothing is sent yet. */
  | { state: "checking"; from: string; to: string }
  /** The pick's write is out. */
  | { state: "saving"; from: string; to: string; sent: Sent };

export function drawingView(snapshot: DrawingSnapshot): DrawingView {
  if (snapshot.matches("inactive")) {
    return { state: "inactive" };
  }
  const { from, refused, sent } = snapshot.context;
  if (from === undefined) {
    throw new Error("an edge is being drawn from no node");
  }
  if (snapshot.matches({ active: { submitting: "previewing" } })) {
    return { state: "checking", from, to: pickedOf(snapshot.context).to };
  }
  if (snapshot.matches({ active: { submitting: "awaiting" } })) {
    return { state: "saving", from, to: sentOf(snapshot.context).to, sent: sentOf(snapshot.context) };
  }
  return { state: "choosing", from, refused, sent };
}
