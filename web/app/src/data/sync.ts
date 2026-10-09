// What the sync chip says (design 9): one state merged from the tick stream, the revision the
// view shows against the one the stream announced, version skew, writes in flight, the
// writes that did not land, and whether the network is up. `summarize` is the pure rule
// (precedence, words, colour); `SyncStatus` collects the inputs and wakes up when a time
// threshold (no flicker under 300ms, behind after 5s, saved for 2s) passes.
import type { StreamStatus, Timers } from "@cairn/client";

import { Emitter } from "./emitter.ts";
import type { Lag } from "./journeys.ts";
import type { Skew } from "./skew.ts";

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

/** A write that did not land, until it is resolved or discarded. */
export interface Problem {
  kind: "conflict" | "rejected" | "failed";
  /** What the reason is, in a sentence. */
  message: string;
  /** What it was about: a node's or journey's title. */
  label: string | undefined;
  /** The screen it happened on, for Go to it (its rejected control is still there). */
  address: string | undefined;
  /** Drops it: the control's own dismiss. */
  discard: () => void;
}

/** Everything the rule reads. */
export interface SyncInput {
  now: number;
  stream: StreamStatus;
  online: boolean;
  demo: boolean;
  skew: Skew | undefined;
  problems: readonly Problem[];
  inFlight: number;
  inFlightSince: number | undefined;
  lag: Lag | undefined;
  /** The last save: when on the monotonic clock, and the wall time to say. */
  saved: { at: number; time: string } | undefined;
  /** The revision of the journey on screen. */
  revision: number | undefined;
}

/** What `SyncStatus` is told, as opposed to what it keeps (the problems) or reads off the clock (now). */
type Reported = Omit<SyncInput, "now" | "problems">;

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

/** The state the chip shows, by the precedence of 9.1: the first that holds. */
export function summarize(input: SyncInput): SyncSummary {
  const { now, lag, problems } = input;
  const lagFor = lag === undefined ? undefined : now - lag.since;
  const conflicts = problems.filter((problem) => problem.kind === "conflict");
  const refused = problems.length - conflicts.length;
  const reason = (problem: Problem | undefined) => (problem === undefined ? "" : `${problem.label === undefined ? "" : `${problem.label}: `}${problem.message}`);
  if (conflicts.length > 0) {
    return { state: "conflict", label: `CONFLICT · ${String(conflicts.length)}`, tone: "act", sentence: reason(conflicts[0]), announce: "assertive" };
  }
  if (refused > 0) {
    return { state: "not-saved", label: `NOT SAVED · ${String(refused)}`, tone: "act", sentence: reason(problems.find((problem) => problem.kind !== "conflict")), announce: "assertive" };
  }
  if (input.skew !== undefined) {
    return { state: "new-version", label: "NEW VERSION · RELOAD", tone: "wait", sentence: "Cairn was updated. Reload to keep saving. Your unsent edits are kept.", announce: "polite" };
  }
  if (!input.online) {
    return { state: "offline", label: "OFFLINE", tone: "wait", sentence: "You are offline. Changes cannot be saved until you are back.", announce: "polite" };
  }
  if (input.stream === "reconnecting") {
    return { state: "reconnecting", label: "RECONNECTING", tone: "wait", sentence: "Live updates paused. Your edits still save.", announce: "polite" };
  }
  if (lag !== undefined && lagFor !== undefined && (lag.failed || lagFor >= SYNC_BEHIND_AFTER_MS)) {
    const newer = lag.announced > lag.shown;
    return {
      state: "behind",
      label: newer ? `BEHIND · REV ${String(lag.announced)}` : "BEHIND",
      tone: "wait",
      sentence: `${newer ? `Showing revision ${String(lag.shown)}; ${String(lag.announced)} exists.` : "Showing older data than exists."} Retrying. Your edits still save and re-base. Click to retry now.`,
      announce: "polite",
    };
  }
  if (input.inFlight > 0 && input.inFlightSince !== undefined && now - input.inFlightSince >= SYNC_SHOW_AFTER_MS) {
    return { state: "saving", label: "SAVING", tone: "working", sentence: `Sending ${plural(input.inFlight, "change")}`, announce: undefined };
  }
  if (lag !== undefined && lagFor !== undefined && lagFor >= SYNC_SHOW_AFTER_MS) {
    return { state: "updating", label: "UPDATING", tone: "working", sentence: lag.announced > lag.shown ? `Loading revision ${String(lag.announced)}` : "Loading the latest changes", announce: undefined };
  }
  if (input.saved !== undefined && now - input.saved.at < SYNC_SAVED_MS) {
    return { state: "saved", label: "SAVED", tone: "good", sentence: `Saved at ${input.saved.time}`, announce: undefined };
  }
  if (input.demo) {
    return { state: "demo", label: "DEMO", tone: "hollow", sentence: "Sample data in this tab. Nothing persists after a reload.", announce: undefined };
  }
  const live = input.stream === "live" ? " · live" : "";
  const revision = input.revision === undefined ? "" : ` · revision ${String(input.revision)}`;
  return { state: "in-sync", label: "IN SYNC", tone: "good", sentence: `In sync${live}${revision}`, announce: undefined };
}

/** What the stores around it tell the chip; each setter is a source of the chip's input. */
export class SyncStatus extends Emitter {
  readonly #timers: Timers;
  readonly #clock: () => number;
  readonly #problems = new Map<string, Problem>();
  #input: Reported;
  #wake: unknown;
  #snapshot: SyncSummary;
  #problemList: readonly Problem[] = [];

  /** `clock` is monotonic milliseconds (the page's `Date` may be held still by a test). */
  constructor(timers: Timers, clock: () => number, demo: boolean) {
    super();
    this.#timers = timers;
    this.#clock = clock;
    this.#input = {
      stream: "idle",
      online: true,
      demo,
      skew: undefined,
      inFlight: 0,
      inFlightSince: undefined,
      lag: undefined,
      saved: undefined,
      revision: undefined,
    };
    this.#snapshot = this.#compute();
  }

  /** The chip's state now. A new object only when it changed, so React re-renders only then. */
  get summary(): SyncSummary {
    return this.#snapshot;
  }

  /** The writes that did not land, in the order they happened. */
  get problems(): readonly Problem[] {
    return this.#problemList;
  }

  /** Counts the write `work` as in flight until it settles. */
  async track<T>(work: Promise<T>): Promise<T> {
    this.#update({ inFlight: this.#input.inFlight + 1, inFlightSince: this.#input.inFlight === 0 ? this.#clock() : this.#input.inFlightSince });
    try {
      return await work;
    } finally {
      const inFlight = this.#input.inFlight - 1;
      this.#update({ inFlight, inFlightSince: inFlight === 0 ? undefined : this.#input.inFlightSince });
    }
  }

  /** A save landed. */
  saved(time: string): void {
    this.#update({ saved: { at: this.#clock(), time } });
  }

  /** Keeps `problem` under `key` until it is resolved: a later attempt for the same control replaces it. */
  problem(key: string, problem: Problem): void {
    this.#problems.set(key, problem);
    this.#changed();
  }

  resolve(key: string): void {
    if (this.#problems.delete(key)) {
      this.#changed();
    }
  }

  setStream(stream: StreamStatus): void {
    this.#update({ stream });
  }

  setLag(lag: Lag | undefined, revision: number | undefined): void {
    this.#update({ lag, revision });
  }

  setSkew(skew: Skew | undefined): void {
    this.#update({ skew });
  }

  setOnline(online: boolean): void {
    this.#update({ online });
  }

  /** The list of problems changed, which the chip's own words may not show (a second rejection under a conflict). */
  #changed(): void {
    this.#problemList = [...this.#problems.values()];
    this.#snapshot = this.#compute();
    this.emit();
    this.#refresh();
  }

  #update(next: Partial<Reported>): void {
    this.#input = { ...this.#input, ...next };
    this.#refresh();
  }

  #compute(): SyncSummary {
    return summarize({ ...this.#input, now: this.#clock(), problems: this.#problemList });
  }

  /** Recomputes, tells listeners if the chip changed, and sleeps until the next threshold. */
  #refresh(): void {
    const next = this.#compute();
    const previous = this.#snapshot;
    if (next.state !== previous.state || next.label !== previous.label || next.sentence !== previous.sentence) {
      this.#snapshot = next;
      this.emit();
    }
    this.#timers.clear(this.#wake);
    this.#wake = undefined;
    const now = this.#clock();
    const { inFlightSince, lag, saved } = this.#input;
    const due = [
      inFlightSince === undefined ? undefined : inFlightSince + SYNC_SHOW_AFTER_MS,
      lag === undefined ? undefined : lag.since + SYNC_SHOW_AFTER_MS,
      lag === undefined ? undefined : lag.since + SYNC_BEHIND_AFTER_MS,
      saved === undefined ? undefined : saved.at + SYNC_SAVED_MS,
    ].filter((at): at is number => at !== undefined && at > now);
    if (due.length > 0) {
      this.#wake = this.#timers.set(() => {
        this.#refresh();
      }, Math.min(...due) - now);
    }
  }
}
