// The tab's one data layer over either host (ARCHITECTURE, Web UI): the capabilities, the
// deployment context, the journey index, the derived journeys, the tick stream that keeps
// them current (H6), the version-skew latch, the notices, and the write path (H5, D7).
import { Subscription, type Timers } from "@cairn/client";

import { DeploymentStore } from "./deployment-store.ts";
import type { Capabilities, Deriver, Host } from "./host.ts";
import { IndexStore } from "./index-store.ts";
import { JourneyStore } from "./journeys.ts";
import { Notices } from "./notices.ts";
import { SkewLatch } from "./skew.ts";
import { ROLLOVER_CHECK_MS } from "./today.ts";
import { write, type WriteIntent, type WriteResult } from "./writes.ts";

export interface SessionParts {
  host: Host;
  deriver: Deriver;
  timers?: Timers;
  /** The page's clock, for date rollover. */
  now?: () => Date;
}

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
  readonly notices = new Notices();
  readonly deployment: DeploymentStore;
  readonly index: IndexStore;
  readonly journeys: JourneyStore;
  readonly #timers: Timers;
  readonly #now: () => Date;
  #rolloverTimer: unknown;

  private constructor(parts: SessionParts, capabilities: Capabilities) {
    this.host = parts.host;
    this.deriver = parts.deriver;
    this.capabilities = capabilities;
    this.#timers = parts.timers ?? realTimers;
    this.#now = parts.now ?? (() => new Date());
    this.subscription = new Subscription(parts.host.openTicks, this.#timers);
    this.deployment = new DeploymentStore(parts.host, this.subscription);
    this.index = new IndexStore(parts.host, this.subscription);
    this.journeys = new JourneyStore({
      host: parts.host,
      deriver: parts.deriver,
      subscription: this.subscription,
      skew: this.skew,
      timers: this.#timers,
    });
    this.#scheduleRollover();
  }

  /** Starts a session over `parts.host`: its capabilities and deployment context first. */
  static async start(parts: SessionParts): Promise<Session> {
    const session = new Session(parts, await parts.host.capabilities());
    await session.deployment.refetch();
    return session;
  }

  /** The shared write path (writes.ts). */
  write(intent: WriteIntent): Promise<WriteResult> {
    return write(this.host, this.skew, this.notices, intent);
  }

  /** Stops the stream and the rollover check. */
  close(): void {
    this.subscription.close();
    this.#timers.clear(this.#rolloverTimer);
  }

  #scheduleRollover(): void {
    this.#rolloverTimer = this.#timers.set(() => {
      this.journeys.rollover(this.#now());
      this.#scheduleRollover();
    }, ROLLOVER_CHECK_MS);
  }
}
