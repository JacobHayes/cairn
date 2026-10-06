// The journeys a tab shows, each derived in the derive worker (ARCHITECTURE, Web UI: data
// flow): one fetch of the domain document, posted to the worker once per revision, derived
// there, and its derive read back. A view stays current (H6): a journey tick newer than the
// revision held refetches it, as does a deployment tick newer than the deployment its
// document was derived with (owners and entities feed rank) and the date passing in the
// deployment's zone. A document from another engine version latches the tab into skew
// instead of being derived. Revisions only move forward: an older document is never shown.
import {
  REOPEN_DELAY_FIRST_MS,
  REOPEN_DELAY_MAX_MS,
  Tracker,
  type RevisionOf,
  type Schema,
  type Subscription,
  type Tick,
  type Timers,
} from "@cairn/client";
import { HostFailure, type DerivationKey, type Derived } from "@cairn/wasm";

import { Emitter } from "./emitter.ts";
import { Missing, type Deriver, type DomainDocument, type Host } from "./host.ts";
import type { SkewLatch } from "./skew.ts";
import { todayIn } from "./today.ts";

export type Journey = Schema<"Journey">;
export type DeriveInputs = Schema<"DeriveInputs">;

/** What a journey's view shows. */
export type JourneyView =
  | { status: "loading" }
  | { status: "missing" }
  | { status: "failed"; message: string }
  | { status: "skew" }
  | { status: "ready"; journey: Journey; inputs: DeriveInputs; key: DerivationKey; derived: Derived };

/**
 * Journeys kept derived after their view closes, so going back to one shows it at once
 * (and refetches it only if it moved meanwhile); the least recently shown beyond this are
 * released from the worker.
 */
export const PARKED_JOURNEY_COUNT_MAX = 8;

const LOADING: JourneyView = { status: "loading" };

/** What a document must reach to be current: its revision, its deployment's, its today. */
interface Want {
  revision: number;
  deployment: number;
  today: string | undefined;
}

/** What the view holds. */
interface Held {
  revision: number;
  deployment: number;
  today: string;
  timezone: string;
}

interface Entry {
  id: string;
  view: JourneyView;
  held: Held | undefined;
  want: Want;
  dirty: boolean;
  running: boolean;
  mounted: number;
  used: number;
  retryDelay: number;
  retryTimer: unknown;
  rolledTo: string | undefined;
}

export interface JourneyStoreParts {
  host: Host;
  deriver: Deriver;
  subscription: Subscription;
  skew: SkewLatch;
  timers: Timers;
}

export const journeyOf = (id: string): RevisionOf => ({ domain: { journey: id } });

/**
 * Whether what the view holds is current for `want`: the journey revision is the tracker's
 * (H6, the one held now), the deployment's and the today are the document's.
 */
function satisfied(revision: number | undefined, held: Held | undefined, want: Want): boolean {
  return (
    revision !== undefined &&
    held !== undefined &&
    revision >= want.revision &&
    held.deployment >= want.deployment &&
    (want.today === undefined || held.today === want.today)
  );
}

export class JourneyStore extends Emitter {
  readonly #parts: JourneyStoreParts;
  readonly #tracker = new Tracker();
  readonly #entries = new Map<string, Entry>();
  #uses = 0;

  constructor(parts: JourneyStoreParts) {
    super();
    this.#parts = parts;
    parts.subscription.listen({
      opened: () => {
        this.#tracker.reopened();
      },
      tick: (tick) => {
        this.#heard(tick);
      },
    });
  }

  /** What journey `id`'s view shows now. */
  view(id: string): JourneyView {
    return this.#entries.get(id)?.view ?? LOADING;
  }

  /** Shows journey `id` until the returned function is called: watched, fetched, derived. */
  mount(id: string): () => void {
    const entry = this.#entry(id);
    entry.mounted += 1;
    entry.used = ++this.#uses;
    const unwatch = this.#parts.subscription.watch([`journey:${id}`]);
    if (entry.held === undefined) {
      this.#request(entry, {});
    }
    return () => {
      unwatch();
      entry.mounted -= 1;
      this.#evict();
    };
  }

  /** Date rollover: refetches every held journey whose today has passed in its zone. */
  rollover(now: Date): void {
    for (const entry of this.#entries.values()) {
      const held = entry.held;
      if (held === undefined) {
        continue;
      }
      let today: string;
      try {
        today = todayIn(held.timezone, now);
      } catch {
        continue;
      }
      if (today !== held.today && today !== entry.rolledTo) {
        entry.rolledTo = today;
        this.#request(entry, { today });
      }
    }
  }

  #entry(id: string): Entry {
    let entry = this.#entries.get(id);
    if (entry === undefined) {
      entry = {
        id,
        view: LOADING,
        held: undefined,
        want: { revision: 0, deployment: 0, today: undefined },
        dirty: false,
        running: false,
        mounted: 0,
        used: 0,
        retryDelay: REOPEN_DELAY_FIRST_MS,
        retryTimer: undefined,
        rolledTo: undefined,
      };
      this.#entries.set(id, entry);
    }
    return entry;
  }

  /** H6: a journey tick newer than held, or a deployment tick newer than a document's. */
  #heard(tick: Tick): void {
    if ("proposal" in tick.of) {
      return;
    }
    const domain = tick.of.domain;
    if (domain === "deployment") {
      for (const entry of this.#entries.values()) {
        if (entry.held !== undefined && entry.held.deployment < tick.revision) {
          this.#request(entry, { deployment: tick.revision });
        }
      }
      return;
    }
    const entry = "journey" in domain ? this.#entries.get(domain.journey) : undefined;
    if (entry === undefined) {
      return;
    }
    if (tick.revision === 0 && entry.held !== undefined) {
      this.#gone(entry);
    } else if (this.#tracker.ticked(tick)) {
      this.#request(entry, { revision: tick.revision });
    }
  }

  #request(entry: Entry, want: Partial<Want>): void {
    entry.want = {
      revision: Math.max(entry.want.revision, want.revision ?? 0),
      deployment: Math.max(entry.want.deployment, want.deployment ?? 0),
      today: want.today ?? entry.want.today,
    };
    entry.dirty = true;
    if (!entry.running) {
      void this.#run(entry);
    }
  }

  /** Fetches until the view holds what was asked for, one fetch at a time per journey. */
  async #run(entry: Entry): Promise<void> {
    entry.running = true;
    while (entry.dirty && !this.#parts.skew.latched && this.#holds(entry)) {
      entry.dirty = false;
      if (satisfied(this.#tracker.held(journeyOf(entry.id)), entry.held, entry.want)) {
        continue;
      }
      try {
        await this.#fetch(entry);
        entry.want.today = undefined;
        entry.retryDelay = REOPEN_DELAY_FIRST_MS;
      } catch (thrown) {
        this.#failed(entry, thrown);
      }
    }
    entry.running = false;
  }

  async #fetch(entry: Entry): Promise<void> {
    const { host, deriver, skew } = this.#parts;
    const text = await host.documentText(entry.id);
    const document = JSON.parse(text) as DomainDocument;
    if (document.engine_version !== host.engineVersion) {
      skew.latch({ document: document.engine_version, engine: host.engineVersion });
      this.#show(entry, entry.view.status === "ready" ? entry.view : { status: "skew" });
      return;
    }
    const revision = document.journey.revision;
    const deployment = document.inputs.deployment.revision;
    const held = entry.held;
    if (held !== undefined && (revision < held.revision || deployment < held.deployment)) {
      return;
    }
    if (!this.#holds(entry)) {
      return;
    }
    const key = await deriver.load(text);
    if (!this.#holds(entry)) {
      void deriver.release(entry.id).catch(() => undefined);
      return;
    }
    const derived = await deriver.derived(entry.id);
    const { today, timezone } = document.inputs;
    entry.held = { revision, deployment, today, timezone };
    this.#tracker.fetched(journeyOf(entry.id), revision);
    this.#show(entry, { status: "ready", journey: document.journey, inputs: document.inputs, key, derived });
  }

  #failed(entry: Entry, thrown: unknown): void {
    if (thrown instanceof Missing) {
      this.#gone(entry);
      return;
    }
    if (thrown instanceof HostFailure && thrown.reason.error === "version_skew") {
      const { document, engine } = thrown.reason;
      this.#parts.skew.latch({ document, engine });
      return;
    }
    this.#tracker.refetchFailed(journeyOf(entry.id));
    if (entry.view.status !== "ready") {
      this.#show(entry, { status: "failed", message: thrown instanceof Error ? thrown.message : String(thrown) });
    }
    const delay = entry.retryDelay;
    entry.retryDelay = Math.min(delay * 2, REOPEN_DELAY_MAX_MS);
    this.#parts.timers.clear(entry.retryTimer);
    entry.retryTimer = this.#parts.timers.set(() => {
      this.#request(entry, {});
    }, delay);
  }

  /** The journey no longer exists: deleted, or never there. */
  #gone(entry: Entry): void {
    entry.held = undefined;
    this.#tracker.forget(journeyOf(entry.id));
    void this.#parts.deriver.release(entry.id).catch(() => undefined);
    this.#show(entry, { status: "missing" });
  }

  /** Whether `entry` is still the store's (not released while a fetch was in flight). */
  #holds(entry: Entry): boolean {
    return this.#entries.get(entry.id) === entry;
  }

  #show(entry: Entry, view: JourneyView): void {
    entry.view = view;
    this.emit();
  }

  /** Releases the least recently shown parked journeys beyond the bound. */
  #evict(): void {
    const parked = [...this.#entries.values()].filter((entry) => entry.mounted === 0);
    parked.sort((left, right) => right.used - left.used);
    for (const entry of parked.slice(PARKED_JOURNEY_COUNT_MAX)) {
      this.#entries.delete(entry.id);
      this.#parts.timers.clear(entry.retryTimer);
      this.#tracker.forget(journeyOf(entry.id));
      void this.#parts.deriver.release(entry.id).catch(() => undefined);
    }
  }
}
