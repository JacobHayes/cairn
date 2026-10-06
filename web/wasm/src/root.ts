// The in-browser host (ARCHITECTURE, Web UI: in-browser host): the module's composition root
// (the service over the memory store seeded from the fixtures, the in-process notifier, one
// local identity) with the API's JSON for its answers, and the loop that hands subscribers
// their ticks (H6): after every write, and again when the coalescing interval asks it to wait.
import type { Schema } from "@cairn/client";

import { BrowserRoot, type RootSubscription } from "../generated/cairn_wasm.js";

import { loadEngine, type Engine, type WasmSource } from "./engine.ts";
import { hosted, parsed, stoppedBy, type HistoryPage, type PatchRequest, type Taken } from "./types.ts";

/** Hears a subscription's ticks: the first time, the current revisions (`first`). */
export type TickListener = (ticks: Schema<"Tick">[], first: boolean) => void;

/** The clock writes and documents are made at: an RFC 3339 timestamp. */
export type Clock = () => string;

interface Subscriber {
  subscription: RootSubscription;
  listener: TickListener;
  timer: ReturnType<typeof setTimeout> | undefined;
}

/** The in-browser host over its seeded root. */
export class InBrowserHost {
  readonly engine: Engine;
  readonly #root: BrowserRoot;
  readonly #clock: Clock;
  readonly #subscribers = new Set<Subscriber>();

  private constructor(engine: Engine, root: BrowserRoot, clock: Clock) {
    this.engine = engine;
    this.#root = root;
    this.#clock = clock;
  }

  /** Loads the module at `wasm` in this page and seeds a root (fixtures, on every load). */
  static async start(wasm: WasmSource, clock: Clock = () => new Date().toISOString()): Promise<InBrowserHost> {
    const engine = await loadEngine(wasm);
    return new InBrowserHost(engine, hosted(() => new BrowserRoot()), clock);
  }

  /** The capabilities document. */
  capabilities(): Schema<"Capabilities"> {
    return parsed<Schema<"Capabilities">>(hosted(() => this.#root.capabilities()));
  }

  /** The journey index (C16). */
  journeys(): Schema<"JourneyPage"> {
    return parsed<Schema<"JourneyPage">>(hosted(() => this.#root.journeys()));
  }

  /** A journey's domain document now, as the server answers it: the text to derive. */
  documentText(journey: string): string {
    return hosted(() => this.#root.document(journey, this.#clock()));
  }

  /**
   * J4: a page of a journey's history, or of `node`'s, after the log position `after`, as
   * `GET /journeys/{id}/history` answers it.
   */
  history(journey: string, node?: string, after?: number): HistoryPage {
    return parsed<HistoryPage>(hosted(() => this.#root.history(journey, node ?? "", after ?? -1)));
  }

  /** The deployment (E6). */
  deployment(): Schema<"Deployment"> {
    return parsed<Schema<"Deployment">>(hosted(() => this.#root.deployment()));
  }

  /**
   * Submits a patch as the local user: the API's patch answer, or a HostFailure whose reason
   * is the rejection (stale with what intervened, invalid, or a reused id). Subscribers hear
   * what it moved.
   */
  patch(request: PatchRequest): Schema<"PatchAnswer"> {
    const answer = parsed<Schema<"PatchAnswer">>(
      hosted(() => this.#root.patch(JSON.stringify(request), this.#clock())),
    );
    this.#deliverAll();
    return answer;
  }

  /**
   * H6: subscribes `listener` to what `watching` names (`deployment`, `journey:<id>`,
   * `route:<id>`, `proposal:<id>`, `journeys`, `routes`, `proposals`). It hears the current
   * revisions first, then each move. The returned function unsubscribes.
   */
  subscribe(watching: string[], listener: TickListener): () => void {
    const subscription = hosted(() => this.#root.subscribe(JSON.stringify(watching)));
    const subscriber: Subscriber = { subscription, listener, timer: undefined };
    this.#subscribers.add(subscriber);
    queueMicrotask(() => {
      this.#deliver(subscriber);
    });
    return () => {
      clearTimeout(subscriber.timer);
      this.#subscribers.delete(subscriber);
      if (stoppedBy()?.error !== "aborted") {
        subscriber.subscription.free();
      }
    };
  }

  #deliverAll(): void {
    for (const subscriber of this.#subscribers) {
      this.#deliver(subscriber);
    }
  }

  /** Hands a subscriber what it holds, or comes back when the coalescing interval ends. */
  #deliver(subscriber: Subscriber): void {
    if (!this.#subscribers.has(subscriber) || subscriber.timer !== undefined) {
      return;
    }
    const now = performance.now();
    let taken: Taken;
    try {
      taken = parsed<Taken>(hosted(() => subscriber.subscription.take(now)));
    } catch {
      // The module stopped: nothing more is announced until the page reloads.
      this.#subscribers.delete(subscriber);
      return;
    }
    switch (taken.take) {
      case "current":
      case "ticks":
        subscriber.listener(taken.ticks, taken.take === "current");
        return;
      case "wait":
        subscriber.timer = setTimeout(() => {
          subscriber.timer = undefined;
          this.#deliver(subscriber);
        }, Math.max(0, taken.until_ms - now));
        return;
      case "empty":
        return;
    }
  }
}
