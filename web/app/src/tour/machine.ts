// PRD: C20.
// The tour's lifecycle as one machine (PRACTICES, frontend state): who owns the practice
// journey, and what is true of the person's record while it is made, walked and removed. The
// record is written before the journey exists and names it until it is gone, so a journey the
// tour made is never lost track of; every write names the record it expects, so two tabs end up
// on one run and an old run's ending cannot undo a newer run. The screens only render this
// state; what a person sees on the way (steps, the band) is `Tour.tsx`.
//
// Facts the page knows and the machine cannot are events: `journey` (the practice journey is
// there, or confirmed not to be) and `where` (the location is under the practice journey).
import { assign, fromPromise, setup, type SnapshotFrom } from "xstate";

import type { Tour, TourWrite, TourWriteRequest } from "../data/host.ts";

/** What the tour does to the world; the host and the write path in the app, controlled promises in the tests. */
export interface TourPorts {
  read: () => Promise<Tour>;
  write: (request: TourWriteRequest) => Promise<TourWrite>;
  /** A fresh id for a practice journey. */
  newJourneyId: () => string;
  /** Creates practice journey `id` as of `today`; true when it exists afterwards. */
  create: (id: string, today: string) => Promise<boolean>;
  /** Deletes practice journey `id`: it was already gone, was deleted, or could not be. */
  remove: (id: string) => Promise<"gone" | "deleted" | "failed">;
}

export type TourEnding = "finished" | "dismissed";

export interface TourContext {
  ports: TourPorts;
  /** The record as last read or written. */
  tour: Tour | undefined;
  /** The practice journey of the run in hand. */
  run: string | undefined;
  /** How the run is ending. */
  ending: TourEnding | undefined;
  /** What failed, while a failure is shown: the start, or taking the practice journey away. */
  failure: "start" | "cleanup" | undefined;
  /** A closing write (`ending`) did not land: the tab keeps the offer shut and You offers to retry. */
  unsaved: boolean;
  /** The person asked for the tour (the rail's link, You), whatever their record says. */
  invited: boolean;
  /** Whether the location is under the practice journey. */
  under: boolean;
}

export type TourEvent =
  | { type: "journey"; id: string; exists: boolean }
  | { type: "where"; under: boolean }
  | { type: "invite" }
  | { type: "uninvite" }
  | { type: "start" }
  | { type: "dismiss" }
  | { type: "retry" }
  | { type: "save" }
  | { type: "close" }
  | { type: "finish" }
  | { type: "skip" }
  | { type: "reload" };

type Written = { kind: "ok" } | { kind: "conflict"; current: Tour } | { kind: "failed" };

function written(result: TourWrite): Written {
  if (result.ok) {
    return { kind: "ok" };
  }
  return "current" in result ? { kind: "conflict", current: result.current } : { kind: "failed" };
}

/** The record `tour` is, as a write names it when it expects it. */
export function expecting(tour: Tour | undefined): TourWriteRequest["expect"] {
  return tour === undefined || tour.state === "none" ? { state: "none" } : { state: tour.state, ...(tour.practice_journey == null ? {} : { practice_journey: tour.practice_journey }) };
}

const recordOf = (tour: Tour, state: Tour["state"], journey?: string): Tour => ({ today: tour.today, state, ...(journey === undefined ? {} : { practice_journey: journey }) });

/** How long a journey missing under a started record is waited for before the run counts as abandoned. */
export const PENDING_MS = 5000;

export const tourMachine = setup({
  types: { context: {} as TourContext, events: {} as TourEvent, input: {} as { ports: TourPorts } },
  actors: {
    read: fromPromise<Tour, { ports: TourPorts }>(({ input }) => input.ports.read()),
    // Writes answer a value for every outcome: a thrown host failure is a failed write too.
    write: fromPromise<Written, { ports: TourPorts; request: TourWriteRequest }>(async ({ input }) => {
      try {
        return written(await input.ports.write(input.request));
      } catch {
        return { kind: "failed" };
      }
    }),
    create: fromPromise<boolean, { ports: TourPorts; id: string; today: string }>(async ({ input }) => {
      try {
        return await input.ports.create(input.id, input.today);
      } catch {
        return false;
      }
    }),
    remove: fromPromise<"gone" | "deleted" | "failed", { ports: TourPorts; id: string }>(async ({ input }) => {
      try {
        return await input.ports.remove(input.id);
      } catch {
        return "failed";
      }
    }),
  },
  guards: {
    started: ({ context }) => context.tour?.state === "started",
    ok: ({ event }) => (event as unknown as { output: Written }).output.kind === "ok",
    conflict: ({ event }) => (event as unknown as { output: Written }).output.kind === "conflict",
    created: ({ event }) => (event as unknown as { output: boolean }).output,
    removed: ({ event }) => (event as unknown as { output: string }).output !== "failed",
    isRun: ({ context, event }) => event.type === "journey" && event.id === (context.run ?? context.tour?.practice_journey),
    runIsThere: ({ context, event }) => event.type === "journey" && event.exists && event.id === (context.run ?? context.tour?.practice_journey),
    runIsGone: ({ context, event }) => event.type === "journey" && !event.exists && event.id === (context.run ?? context.tour?.practice_journey),
    away: ({ context }) => !context.under,
    finishing: ({ context }) => context.ending === "finished",
    unsavedEnding: ({ context }) => context.unsaved && context.ending !== undefined,
    failedStart: ({ context }) => context.failure === "start",
  },
  actions: {
    keepWhere: assign({ under: ({ event }) => (event.type === "where" ? event.under : false) }),
    invite: assign({ invited: true }),
    uninvite: assign({ invited: false }),
    // A conflict answers the record now: continue from it.
    adopt: assign({ tour: ({ event }) => (event as unknown as { output: Written & { kind: "conflict" } }).output.current, invited: false, unsaved: false }),
    failStart: assign({ failure: "start" }),
    failCleanup: assign({ failure: "cleanup" }),
    clearFailure: assign({ failure: undefined }),
    reserve: assign({ run: ({ context }) => context.ports.newJourneyId() }),
    started: assign({ tour: ({ context }) => (context.tour === undefined || context.run === undefined ? context.tour : recordOf(context.tour, "started", context.run)), invited: false, unsaved: false }),
    runIsThe: assign({ run: ({ context }) => context.tour?.practice_journey ?? undefined }),
    noRun: assign({ run: undefined }),
    endFinished: assign({ ending: "finished", unsaved: false }),
    endDismissed: assign({ ending: "dismissed", unsaved: false }),
    recorded: assign({
      tour: ({ context }) => (context.tour === undefined || context.ending === undefined ? context.tour : recordOf(context.tour, context.ending)),
      run: undefined,
      failure: undefined,
      invited: false,
      unsaved: false,
    }),
    // The cleanup is done and the record is not: the run is gone from this tab, the record stays as read.
    over: assign({ run: undefined, failure: undefined, invited: false }),
    unsaved: assign({ unsaved: true, invited: false }),
    // A closing write that did not land stays what to retry.
    done: assign({ ending: ({ context }) => (context.unsaved ? context.ending : undefined) }),
  },
}).createMachine({
  id: "tour",
  context: ({ input }) => ({ ports: input.ports, tour: undefined, run: undefined, ending: undefined, failure: undefined, unsaved: false, invited: false, under: false }),
  initial: "reading",
  // Where the location is, and the invitation, are noted in every state.
  on: { where: { actions: "keepWhere" } },
  states: {
    reading: {
      invoke: { src: "read", input: ({ context }) => ({ ports: context.ports }), onDone: { target: "settled", actions: assign({ tour: ({ event }) => event.output }) }, onError: "unreadable" },
    },
    // A failed read is unknown, which offers nothing; a later visit to Home reads again.
    unreadable: { on: { reload: "reading" } },
    settled: {
      always: [{ guard: "started", target: "checking", actions: "runIsThe" }, { target: "idle", actions: "noRun" }],
    },
    // A started record: the run goes on if its journey is there, and is over if it confirmed not to be.
    // Missing is not final at once: another tab may have reserved it a moment before its journey commits, or have just ended it. The page asks again while this waits, and the record is read again after.
    checking: {
      initial: "asking",
      states: {
        asking: { on: { journey: [{ guard: "runIsThere", target: "#tour.running.live" }, { guard: "runIsGone", target: "pending" }] } },
        pending: {
          after: { [PENDING_MS]: "#tour.rereading" },
          on: { journey: { guard: "runIsThere", target: "#tour.running.live" } },
        },
      },
    },
    // The record again, once the wait is over: an ending another tab wrote is adopted; a reservation still there with no journey is an abandoned run.
    rereading: {
      invoke: {
        src: "read",
        input: ({ context }) => ({ ports: context.ports }),
        onDone: { target: "idle", actions: [assign({ tour: ({ event }) => event.output }), "noRun"] },
        onError: { target: "idle", actions: "noRun" },
      },
    },
    idle: {
      on: {
        invite: { actions: "invite" },
        uninvite: { actions: "uninvite" },
        start: "starting",
        dismiss: { target: "saving", actions: "endDismissed" },
        save: { guard: "unsavedEnding", target: "saving" },
        reload: "reading",
      },
    },
    // The record is written to `ending`: Not now, or a closing write that did not land the first time.
    saving: {
      invoke: {
        src: "write",
        input: ({ context }) => ({ ports: context.ports, request: { state: context.ending ?? "dismissed", expect: expecting(context.tour) } }),
        onDone: [{ guard: "ok", target: "idle", actions: ["recorded", "done"] }, { guard: "conflict", target: "settled", actions: ["adopt", "done"] }, { target: "idle", actions: "unsaved" }],
      },
    },
    starting: {
      initial: "reserving",
      states: {
        // The record names the journey before it exists.
        reserving: {
          entry: "reserve",
          invoke: {
            src: "write",
            input: ({ context }) => ({ ports: context.ports, request: { state: "started" as const, practice_journey: context.run ?? "", expect: expecting(context.tour) } }),
            onDone: [{ guard: "ok", target: "creating", actions: "started" }, { guard: "conflict", target: "#tour.settled", actions: "adopt" }, { target: "#tour.failed", actions: "failStart" }],
          },
        },
        creating: {
          invoke: {
            src: "create",
            input: ({ context }) => ({ ports: context.ports, id: context.run ?? "", today: context.tour?.today ?? "" }),
            onDone: [{ guard: "created", target: "#tour.running.confirming" }, { target: "#tour.failed", actions: "failStart" }],
            onError: { target: "#tour.failed", actions: "failStart" },
          },
        },
      },
    },
    failed: {
      on: {
        // A start tries again with a new journey and the stale record as the one expected; a removal tries the removal again.
        retry: [{ guard: "failedStart", target: "starting", actions: "clearFailure" }, { target: "ending.deleting", actions: "clearFailure" }],
        close: [{ guard: "failedStart", target: "idle", actions: ["clearFailure", "noRun"] }, { target: "running.live", actions: ["clearFailure", "done"] }],
      },
    },
    running: {
      initial: "live",
      states: {
        // Just made: its journey is being read for the first time, and not being there yet says nothing.
        confirming: { on: { journey: { guard: "runIsThere", target: "live" } } },
        live: {
          on: {
            // Someone else finished or deleted it: the run is over, quietly, and nothing is written.
            journey: { guard: "runIsGone", target: "#tour.checking.pending" },
            finish: { target: "#tour.ending", actions: "endFinished" },
            skip: { target: "#tour.ending", actions: "endDismissed" },
          },
        },
      },
    },
    ending: {
      initial: "leaving",
      states: {
        // Leaves the practice journey first, so its screen never shows a journey that is going.
        leaving: { always: { guard: "away", target: "deleting" } },
        deleting: {
          invoke: {
            src: "remove",
            input: ({ context }) => ({ ports: context.ports, id: context.run ?? "" }),
            onDone: [{ guard: "removed", target: "recording" }, { target: "#tour.failed", actions: "failCleanup" }],
            onError: { target: "#tour.failed", actions: "failCleanup" },
          },
        },
        recording: {
          invoke: {
            src: "write",
            input: ({ context }) => ({ ports: context.ports, request: { state: context.ending ?? "dismissed", expect: expecting(context.tour) } }),
            onDone: [
              { guard: "ok", target: "#tour.closed", actions: "recorded" },
              // A newer run exists: this one's ending changes nothing.
              { guard: "conflict", target: "#tour.settled", actions: ["adopt", "done"] },
              // The cleanup is done; only the record is not: the tab closes the tour and keeps it shut.
              { target: "#tour.closed", actions: ["over", "unsaved"] },
            ],
          },
        },
      },
    },
    // After an ending: the end card for a finish, nothing for a skip.
    closed: {
      always: [{ guard: "finishing", target: "ended" }, { target: "idle", actions: "done" }],
    },
    ended: { on: { close: { target: "idle", actions: "done" } } },
  },
});

export type TourSnapshot = SnapshotFrom<typeof tourMachine>;
