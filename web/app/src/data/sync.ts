// What the sync chip says (design 9): one state merged from the tick stream, the revision the
// view shows against the one the stream announced, version skew, writes in flight, the
// writes that did not land, and whether the network is up. `summarize` is the pure rule
// (precedence): it returns a tagged case that carries each state's data, and `syncWords`
// turns that case into the label, colour, sentence and announcement. The scheduler
// (`syncMachine`) holds only the facts that carry a deadline - how far the shown reads are
// behind, since when writes have been in flight, and when the last save landed - and wakes at
// the next threshold (no flicker under 300ms, behind after 5s, saved for 2s). Everything
// else the chip shows is read from its owner when it renders: the stream, version skew and
// the network, and the writes that did not land and the writes in flight from the edit
// manager. Nothing derived is stored.
import type { HttpFailure, StreamStatus } from "@cairn/client";
import { assign, setup, type ActorRefFrom } from "xstate";

import type { SaveEvent } from "./activity.ts";
import type { Lag } from "./keyed-read.ts";

/** A write has to take this long to show SAVING, and a refetch to show UPDATING (no flicker). */
export const SYNC_SHOW_AFTER_MS = 300;
/** A refetch that takes this long, or fails, means the view is BEHIND. */
export const SYNC_BEHIND_AFTER_MS = 5000;
/** How long SAVED stays after a write lands. */
export const SYNC_SAVED_MS = 2000;

export type SyncState =
  | "conflict"
  | "not-saved"
  | "new-version"
  | "offline"
  | "reconnecting"
  | "behind"
  | "saving"
  | "updating"
  | "saved"
  | "demo"
  | "in-sync";

/**
 * The colour rule (9.2): good is nothing to do, working is steel, wait is amber (it will sort
 * itself out, or it is safe to wait), act is red (your change is not landing until you act).
 */
export type SyncTone = "good" | "working" | "wait" | "act" | "hollow";

/** A write that did not land, until it is resolved or discarded (an event sent to its owner, the edit manager). */
export interface Problem {
  /** What Discard and Retry name: the write's operation id. */
  id: string;
  kind: "conflict" | "rejected" | "failed";
  /** What the reason is, in a sentence. */
  message: string;
  /** The server's own words behind the message, for a Details button: they may name ids, so never shown by default. */
  detail: HttpFailure | undefined;
  /** What it was about: a node's or journey's title. */
  label: string | undefined;
  /** The screen it happened on, for Go to it (its rejected control is still there). */
  address: string | undefined;
  /** Whether Retry is offered: the write may be sent again by hand. */
  retry: boolean;
}

/** Everything the rule reads. */
export interface SyncInput {
  now: number;
  stream: StreamStatus;
  online: boolean;
  demo: boolean;
  skew: boolean;
  problems: readonly Problem[];
  /** The writes out or being resent, and since when (monotonic ms) there have been some. */
  inFlight: { count: number; since: number } | undefined;
  lag: Lag | undefined;
  /** The last save: when on the monotonic clock, and the wall time to say. */
  saved: { at: number; time: string } | undefined;
  /** The revision of the journey on screen. */
  revision: number | undefined;
}

/** The state the chip is in, with the data that state's words are made of. */
export type SyncCase =
  | { state: "conflict"; count: number; first: Problem }
  | { state: "not-saved"; count: number; first: Problem }
  | { state: "new-version" }
  | { state: "offline" }
  | { state: "reconnecting" }
  | { state: "behind"; shown: number; announced: number }
  | { state: "saving"; count: number }
  | { state: "updating"; shown: number; announced: number }
  | { state: "saved"; time: string }
  | { state: "demo" }
  | { state: "in-sync"; live: boolean; revision: number | undefined };

/** What the chip shows of a case. */
export interface SyncSummary {
  state: SyncState;
  /** The chip's always-visible label. */
  label: string;
  tone: SyncTone;
  /** The hover sentence, and the button's accessible name. */
  sentence: string;
  /** How a change to this state is announced, if it is. */
  announce: "polite" | "assertive" | undefined;
}

const plural = (count: number, word: string) => `${String(count)} ${word}${count === 1 ? "" : "s"}`;

/** The state the chip is in, by the precedence of 9.1: the first that holds. */
export function summarize(input: SyncInput): SyncCase {
  const { now, lag, problems } = input;
  const lagFor = lag === undefined ? undefined : now - lag.since;
  const conflicts = problems.filter((problem) => problem.kind === "conflict");
  const refused = problems.filter((problem) => problem.kind !== "conflict");
  const [conflict] = conflicts;
  const [refusal] = refused;
  if (conflict !== undefined) {
    return { state: "conflict", count: conflicts.length, first: conflict };
  }
  if (refusal !== undefined) {
    return { state: "not-saved", count: refused.length, first: refusal };
  }
  if (input.skew) {
    return { state: "new-version" };
  }
  if (!input.online) {
    return { state: "offline" };
  }
  if (input.stream === "reconnecting") {
    return { state: "reconnecting" };
  }
  if (lag !== undefined && lagFor !== undefined && (lag.failed || lagFor >= SYNC_BEHIND_AFTER_MS)) {
    return { state: "behind", shown: lag.shown, announced: lag.announced };
  }
  if (input.inFlight !== undefined && now - input.inFlight.since >= SYNC_SHOW_AFTER_MS) {
    return { state: "saving", count: input.inFlight.count };
  }
  if (lag !== undefined && lagFor !== undefined && lagFor >= SYNC_SHOW_AFTER_MS) {
    return { state: "updating", shown: lag.shown, announced: lag.announced };
  }
  if (input.saved !== undefined && now - input.saved.at < SYNC_SAVED_MS) {
    return { state: "saved", time: input.saved.time };
  }
  if (input.demo) {
    return { state: "demo" };
  }
  return { state: "in-sync", live: input.stream === "live", revision: input.revision };
}

const reason = (problem: Problem) => `${problem.label === undefined ? "" : `${problem.label}: `}${problem.message}`;

/** The words and colour of a case (9.2). */
export function syncWords(shown: SyncCase): SyncSummary {
  switch (shown.state) {
    case "conflict":
      return { state: "conflict", label: `CONFLICT · ${String(shown.count)}`, tone: "act", sentence: reason(shown.first), announce: "assertive" };
    case "not-saved":
      return { state: "not-saved", label: `NOT SAVED · ${String(shown.count)}`, tone: "act", sentence: reason(shown.first), announce: "assertive" };
    case "new-version":
      return { state: "new-version", label: "NEW VERSION · RELOAD", tone: "wait", sentence: "Cairn was updated. Reload to keep saving. Your unsent edits are kept.", announce: "polite" };
    case "offline":
      return { state: "offline", label: "OFFLINE", tone: "wait", sentence: "You are offline. Changes cannot be saved until you are back.", announce: "polite" };
    case "reconnecting":
      return { state: "reconnecting", label: "RECONNECTING", tone: "wait", sentence: "Live updates paused. Your edits still save.", announce: "polite" };
    case "behind": {
      return {
        state: "behind",
        label: "BEHIND",
        tone: "wait",
        sentence: "Showing older data than exists. Retrying. Your edits still save and re-base. Click to retry now.",
        announce: "polite",
      };
    }
    case "saving":
      return { state: "saving", label: "SAVING", tone: "working", sentence: `Sending ${plural(shown.count, "change")}`, announce: undefined };
    case "updating":
      return {
        state: "updating",
        label: "UPDATING",
        tone: "working",
        sentence: "Loading the latest changes",
        announce: undefined,
      };
    case "saved":
      return { state: "saved", label: "SAVED", tone: "good", sentence: `Saved at ${shown.time}`, announce: undefined };
    case "demo":
      return { state: "demo", label: "DEMO", tone: "hollow", sentence: "Sample data in this tab. Nothing persists after a reload.", announce: undefined };
    case "in-sync":
      return {
        state: "in-sync",
        label: "IN SYNC",
        tone: "good",
        sentence: `In sync${shown.live ? " · live" : ""}`,
        announce: undefined,
      };
  }
}

/** What the chip says, for what it reads. */
export const syncSummary = (input: SyncInput): SyncSummary => syncWords(summarize(input));

/** The facts that carry a deadline, which the scheduler holds. */
export interface SyncTiming {
  /** Monotonic ms at the scheduler's last event or wake: a threshold counts from here. */
  now: number;
  /** How far the worst shown read is behind what the stream announced. */
  lag: Lag | undefined;
  /** When the writes in flight first became more than none. */
  inFlightSince: number | undefined;
  /** When the last save landed. */
  saved: number | undefined;
}

/** What the chip reads of its other owners when it renders. */
export interface SyncReading {
  stream: StreamStatus;
  online: boolean;
  demo: boolean;
  skew: boolean;
  revision: number | undefined;
  /** The writes out or being resent, as the edit manager counts them. */
  count: number;
  problems: readonly Problem[];
  /** The last save, which says when it landed. */
  latest: SaveEvent | undefined;
}

/** The wall time of a save, as the chip says it. */
export const clockWords = (at: Date): string => at.toLocaleTimeString([], { hour12: false });

/** The rule's input from the scheduler's facts and the owners'; the one place the in-flight fact is made. */
export function syncInputOf(timing: SyncTiming, read: SyncReading): SyncInput {
  return {
    now: timing.now,
    stream: read.stream,
    online: read.online,
    demo: read.demo,
    skew: read.skew,
    problems: read.problems,
    inFlight: read.count > 0 && timing.inFlightSince !== undefined ? { count: read.count, since: timing.inFlightSince } : undefined,
    lag: timing.lag,
    saved: timing.saved !== undefined && read.latest !== undefined ? { at: timing.saved, time: clockWords(read.latest.at) } : undefined,
    revision: read.revision,
  };
}

/** The earliest moment after `timing.now` at which what the chip says changes by the clock alone, if any. */
export function nextDeadline(timing: SyncTiming): number | undefined {
  const { now, lag, inFlightSince, saved } = timing;
  const due = [
    inFlightSince === undefined ? undefined : inFlightSince + SYNC_SHOW_AFTER_MS,
    lag === undefined ? undefined : lag.since + SYNC_SHOW_AFTER_MS,
    lag === undefined ? undefined : lag.since + SYNC_BEHIND_AFTER_MS,
    saved === undefined ? undefined : saved + SYNC_SAVED_MS,
  ].filter((at): at is number => at !== undefined && at > now);
  return due.length === 0 ? undefined : Math.min(...due);
}

/** What the scheduler is told. */
export type SyncEvent =
  /** The worst lag of the reads the chip counts changed (or there is none). */
  | { type: "lag"; lag: Lag | undefined }
  /** A save landed. */
  | { type: "saved" }
  /** The edit manager's count of writes in flight changed. */
  | { type: "inFlight"; count: number };

const sameLag = (left: Lag | undefined, right: Lag | undefined): boolean =>
  left === right ||
  (left !== undefined && right !== undefined && left.subject === right.subject && left.shown === right.shown && left.announced === right.announced && left.since === right.since && left.failed === right.failed);

/** The facts after `event` at `now`: `timing` itself when the event changes none of them. */
export function timed(timing: SyncTiming, event: SyncEvent, now: number): SyncTiming {
  switch (event.type) {
    case "lag":
      return sameLag(timing.lag, event.lag) ? timing : { ...timing, now, lag: event.lag };
    case "saved":
      return { ...timing, now, saved: now };
    case "inFlight": {
      if (event.count === 0) {
        return timing.inFlightSince === undefined ? timing : { ...timing, now, inFlightSince: undefined };
      }
      return timing.inFlightSince === undefined ? { ...timing, now, inFlightSince: now } : timing;
    }
  }
}

/** A save is announced while it is the latest and its SAVED window runs, whatever the chip says at the moment: "Saved", and the receipt's warning. */
export function savedAnnouncement(latest: SaveEvent | undefined, timing: SyncTiming): { id: number; text: string } | undefined {
  if (latest === undefined || timing.saved === undefined || timing.now - timing.saved >= SYNC_SAVED_MS) {
    return undefined;
  }
  return { id: latest.id, text: ["Saved", latest.warning].filter(Boolean).join(". ") };
}

export interface SyncContext {
  timing: SyncTiming;
  /** Monotonic milliseconds: the clock `lag.since` is measured on. */
  monotonic: () => number;
}

/**
 * The scheduler. Between events it waits for the next deadline (a delayed transition) and wakes
 * to move `timing.now` past it, so a subscriber's render sees the threshold crossed with no
 * event sent; with no deadline it waits for nothing (an infinite delay would fire at once).
 */
export const syncMachine = setup({
  types: { context: {} as SyncContext, events: {} as SyncEvent, input: {} as { monotonic: () => number } },
  guards: {
    due: ({ context }) => nextDeadline(context.timing) !== undefined,
    // A relay that changes no fact takes no transition, so the chip does not re-render for it.
    changes: ({ context, event }) => timed(context.timing, event, context.monotonic()) !== context.timing,
  },
  actions: {
    take: assign(({ context, event }) => ({ timing: timed(context.timing, event, context.monotonic()) })),
    tick: assign(({ context }) => ({ timing: { ...context.timing, now: context.monotonic() } })),
  },
  delays: {
    untilDue: ({ context }) => Math.max(0, (nextDeadline(context.timing) ?? context.timing.now) - context.timing.now),
  },
}).createMachine({
  id: "sync",
  context: ({ input }) => ({ timing: { now: input.monotonic(), lag: undefined, inFlightSince: undefined, saved: undefined }, monotonic: input.monotonic }),
  initial: "deciding",
  states: {
    deciding: { always: [{ guard: "due", target: "waiting" }, { target: "resting" }] },
    waiting: { after: { untilDue: { target: "deciding", actions: "tick" } } },
    resting: {},
  },
  on: {
    lag: { guard: "changes", target: ".deciding", actions: "take" },
    saved: { guard: "changes", target: ".deciding", actions: "take" },
    inFlight: { guard: "changes", target: ".deciding", actions: "take" },
  },
});

export type SyncRef = ActorRefFrom<typeof syncMachine>;
