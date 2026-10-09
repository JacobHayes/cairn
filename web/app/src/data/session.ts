// The tab's one data layer over either host (ARCHITECTURE, Web UI): the capabilities, the
// deployment context, the caller, the derived journeys, the tick stream that keeps
// them current (H6), the version-skew latch, what the tab did (activity), what the sync
// chip says (sync), and the write path (H5, D7).
import { Subscription, type Timers } from "@cairn/client";

import { Activity, type TitleOf } from "./activity.ts";
import { DeploymentStore } from "./deployment-store.ts";
import { readDrafts, writeDraft } from "./drafts.ts";
import type { Capabilities, Deriver, Host } from "./host.ts";
import { JourneyStore } from "./journeys.ts";
import { SkewLatch } from "./skew.ts";
import { SyncStatus } from "./sync.ts";
import { ViewerStore } from "./viewer-store.ts";
import { ROLLOVER_CHECK_MS } from "./today.ts";
import type { ConsequenceLine } from "./activity.ts";
import { reasonOf, recordSave, write, type Rejection, type WriteIntent, type WriteResult } from "./writes.ts";

export interface SessionParts {
  host: Host;
  deriver: Deriver;
  timers?: Timers;
  /** The page's clock, for date rollover. */
  now?: () => Date;
}

/** The page, when there is one: the unit tests run without a window, so no online or offline events. */
const page: Window | undefined = typeof window === "undefined" ? undefined : window;

const realTimers: Timers = {
  set: (callback, ms) => setTimeout(callback, ms),
  clear: (handle) => {
    clearTimeout(handle as ReturnType<typeof setTimeout>);
  },
};

export class Session {
  readonly host: Host;
  /** The derive worker: projections and drafts over the journeys it holds. */
  readonly deriver: Deriver;
  readonly capabilities: Capabilities;
  readonly subscription: Subscription;
  readonly skew = new SkewLatch();
  readonly activity = new Activity();
  readonly sync: SyncStatus;
  readonly deployment: DeploymentStore;
  readonly viewer: ViewerStore;
  readonly journeys: JourneyStore;
  readonly #timers: Timers;
  readonly #now: () => Date;
  #rolloverTimer: unknown;
  readonly #online = {
    online: () => { this.sync.setOnline(true); },
    offline: () => { this.sync.setOnline(false); },
  };

  private constructor(parts: SessionParts, capabilities: Capabilities) {
    this.host = parts.host;
    this.deriver = parts.deriver;
    this.capabilities = capabilities;
    this.#timers = parts.timers ?? realTimers;
    this.#now = parts.now ?? (() => new Date());
    this.sync = new SyncStatus(this.#timers, () => performance.now(), parts.host.kind === "browser");
    this.subscription = new Subscription(parts.host.openTicks, this.#timers);
    this.deployment = new DeploymentStore(parts.host, this.subscription);
    this.viewer = new ViewerStore(parts.host, this.deployment);
    this.journeys = new JourneyStore({
      host: parts.host,
      deriver: parts.deriver,
      subscription: this.subscription,
      skew: this.skew,
      timers: this.#timers,
    });
    this.#scheduleRollover();
    this.#feedSync();
    this.#restoreRejections();
  }

  /** Starts a session over `parts.host`: its capabilities and deployment context first. */
  static async start(parts: SessionParts): Promise<Session> {
    const session = new Session(parts, await parts.host.capabilities());
    await session.deployment.refetch();
    session.viewer.refetch();
    return session;
  }

  /** A node's title in a journey this tab holds, or its key. */
  readonly titleOf: TitleOf = (journey, node) => {
    const view = this.journeys.view(journey);
    return (view.status === "ready" ? view.journey.graph.nodes?.find((each) => each.key === node)?.title : undefined) ?? node;
  };

  /** Records a save that did not go through `write` (an apply, an import); its receipt warning, if any. */
  recordSave(text: string, lines: ConsequenceLine[]): string | undefined {
    return recordSave({ activity: this.activity, sync: this.sync, titleOf: this.titleOf }, text, lines);
  }

  /** The shared write path (writes.ts). */
  write(intent: WriteIntent): Promise<WriteResult> {
    return write({ host: this.host, skew: this.skew, activity: this.activity, sync: this.sync, titleOf: this.titleOf }, intent);
  }

  /** Stops the stream, the rollover check and the network listeners. */
  close(): void {
    this.subscription.close();
    this.#timers.clear(this.#rolloverTimer);
    for (const [event, listener] of Object.entries(this.#online)) {
      page?.removeEventListener(event, listener);
    }
  }

  /**
   * A rejection kept as a draft survives a reload (ARCHITECTURE, Web UI: drafts), so the chip
   * counts it again before its control is on screen; the control takes it over once it is.
   */
  #restoreRejections(): void {
    for (const [key, kept] of readDrafts<{ rejection: Rejection; address?: string }>("rejected:")) {
      this.sync.problem(key, {
        kind: kept.rejection.rejection === "stale" ? "conflict" : "rejected",
        message: reasonOf(kept.rejection),
        label: undefined,
        address: kept.address,
        discard: () => {
          writeDraft(key, undefined);
          this.sync.resolve(key);
        },
      });
    }
  }

  /** Tells the sync chip what its sources say, now and whenever they change. */
  #feedSync(): void {
    const lag = () => {
      this.sync.setLag(this.journeys.lag, this.journeys.derivation?.revision);
    };
    this.subscription.onStatus(() => {
      this.sync.setStream(this.subscription.status);
    });
    this.skew.subscribe(() => {
      this.sync.setSkew(this.skew.current);
    });
    this.journeys.subscribe(lag);
    for (const [event, listener] of Object.entries(this.#online)) {
      page?.addEventListener(event, listener);
    }
    this.sync.setOnline(page?.navigator.onLine ?? true);
  }

  #scheduleRollover(): void {
    this.#rolloverTimer = this.#timers.set(() => {
      this.journeys.rollover(this.#now());
      this.#scheduleRollover();
    }, ROLLOVER_CHECK_MS);
  }
}
