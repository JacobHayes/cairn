// The journey index a tab shows (C16), kept current (H6): watching every journey, it
// refetches when a tick names a journey at a revision newer than the index lists, or one it
// does not list. Paging and filters land with the index screen (5.5); the shell reads the
// first page. A failed fetch forgets what was asked for, so the next tick asks again.
import { Tracker, type Subscription, type Tick } from "@cairn/client";

import { Emitter } from "./emitter.ts";
import type { Host, JourneyPage } from "./host.ts";
import { journeyOf } from "./journeys.ts";

export type IndexView =
  | { status: "loading" }
  | { status: "failed"; message: string }
  | { status: "ready"; page: JourneyPage };

export class IndexStore extends Emitter {
  readonly #host: Host;
  readonly #subscription: Subscription;
  readonly #tracker = new Tracker();
  #view: IndexView = { status: "loading" };
  #dirty = false;
  #running = false;
  #mounted = 0;

  constructor(host: Host, subscription: Subscription) {
    super();
    this.#host = host;
    this.#subscription = subscription;
    subscription.listen({
      opened: () => {
        this.#tracker.reopened();
      },
      tick: (tick) => {
        this.#heard(tick);
      },
    });
  }

  get view(): IndexView {
    return this.#view;
  }

  /** Shows the index until the returned function is called. */
  mount(): () => void {
    this.#mounted += 1;
    const unwatch = this.#subscription.watch(["journeys"]);
    this.#refetch();
    return () => {
      this.#mounted -= 1;
      unwatch();
    };
  }

  #heard(tick: Tick): void {
    const domain = "domain" in tick.of ? tick.of.domain : undefined;
    if (this.#mounted > 0 && typeof domain === "object" && "journey" in domain && this.#tracker.ticked(tick)) {
      this.#refetch();
    }
  }

  #refetch(): void {
    this.#dirty = true;
    if (!this.#running) {
      void this.#run();
    }
  }

  async #run(): Promise<void> {
    this.#running = true;
    while (this.#dirty) {
      this.#dirty = false;
      try {
        const page = await this.#host.journeys();
        this.#tracker.reopened();
        for (const item of page.items) {
          this.#tracker.fetched(journeyOf(item.id), item.revision);
        }
        this.#view = { status: "ready", page };
      } catch (thrown) {
        const message = thrown instanceof Error ? thrown.message : String(thrown);
        if (this.#view.status !== "ready") {
          this.#view = { status: "failed", message };
        }
        this.#tracker.reopened();
      }
      this.emit();
    }
    this.#running = false;
  }
}
