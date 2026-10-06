// H6's subscription helper: one tick stream per page, watching the union of what its views
// watch, reopened when that union changes and again after a drop. Views listen for the
// stream opening (its first ticks are the current revisions, so each view's Tracker forgets
// what it asked for) and for each tick, which they compare with the revision they hold
// (tracker.ts). The stream itself is a transport: Server-Sent Events from the API
// (`eventSourceTicks`) or the in-browser host's subscriptions, behind `OpenTicks`.
import type { Tick } from "./tracker.ts";

/** What a tick stream tells whoever opened it. */
export interface TickHandlers {
  /** The stream (re)opened: the ticks that follow start with the current revisions. */
  opened(): void;
  tick(tick: Tick): void;
  /** The stream dropped. `closed`: it will not come back by itself. */
  dropped(closed: boolean): void;
}

/** Opens a stream watching `watching`; answers a function that closes it. */
export type OpenTicks = (watching: readonly string[], handlers: TickHandlers) => () => void;

/** What a view hears from the stream. */
export interface TickListener {
  opened(): void;
  tick(tick: Tick): void;
}

export type StreamStatus = "idle" | "connecting" | "live" | "reconnecting";

/**
 * The first wait before reopening a stream that closed, and the longest: PRACTICES' SSE
 * coalescing interval and SSE write stall, the stream's own named limits, so a closed
 * stream is retried no faster than ticks come and no slower than a stalled one is cut.
 */
export const REOPEN_DELAY_FIRST_MS = 250;
export const REOPEN_DELAY_MAX_MS = 15_000;

/** The timer the helper waits with (a test passes its own). */
export interface Timers {
  set(callback: () => void, ms: number): unknown;
  clear(handle: unknown): void;
}

const realTimers: Timers = {
  set: (callback, ms) => setTimeout(callback, ms),
  clear: (handle) => {
    clearTimeout(handle as ReturnType<typeof setTimeout>);
  },
};

/** The page's tick stream. */
export class Subscription {
  readonly #open: OpenTicks;
  readonly #timers: Timers;
  readonly #names = new Map<string, number>();
  readonly #listeners = new Set<TickListener>();
  readonly #statusListeners = new Set<() => void>();
  #close: (() => void) | undefined;
  #watching = "";
  #reopenTimer: unknown;
  #reopenScheduled = false;
  #delay = REOPEN_DELAY_FIRST_MS;
  status: StreamStatus = "idle";

  constructor(open: OpenTicks, timers: Timers = realTimers) {
    this.#open = open;
    this.#timers = timers;
  }

  /** Watches `names` until the returned function is called. */
  watch(names: readonly string[]): () => void {
    for (const name of names) {
      this.#names.set(name, (this.#names.get(name) ?? 0) + 1);
    }
    this.#schedule();
    let released = false;
    return () => {
      if (released) {
        return;
      }
      released = true;
      for (const name of names) {
        const count = (this.#names.get(name) ?? 1) - 1;
        if (count === 0) {
          this.#names.delete(name);
        } else {
          this.#names.set(name, count);
        }
      }
      this.#schedule();
    };
  }

  /** Hears the stream until the returned function is called. */
  listen(listener: TickListener): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  /** Hears status changes until the returned function is called. */
  onStatus(listener: () => void): () => void {
    this.#statusListeners.add(listener);
    return () => this.#statusListeners.delete(listener);
  }

  /** Reopens the stream now, so its current revisions ask every view again (a refetch failed). */
  reopen(): void {
    this.#restart(true);
  }

  /** Closes the stream for good. */
  close(): void {
    this.#names.clear();
    this.#restart(false);
  }

  /** Reopens on the next microtask, so a batch of watch changes opens one stream. */
  #schedule(): void {
    if (this.#reopenScheduled) {
      return;
    }
    this.#reopenScheduled = true;
    queueMicrotask(() => {
      this.#reopenScheduled = false;
      const watching = [...this.#names.keys()].sort().join("\n");
      if (watching !== this.#watching || this.#close === undefined) {
        this.#restart(true);
      }
    });
  }

  #restart(open: boolean): void {
    this.#timers.clear(this.#reopenTimer);
    this.#reopenTimer = undefined;
    this.#close?.();
    this.#close = undefined;
    const names = [...this.#names.keys()].sort();
    this.#watching = names.join("\n");
    if (!open || names.length === 0) {
      this.#setStatus("idle");
      return;
    }
    this.#setStatus(this.status === "idle" ? "connecting" : "reconnecting");
    let current = true;
    const handlers: TickHandlers = {
      opened: () => {
        if (current) {
          this.#opened();
        }
      },
      tick: (tick) => {
        if (current) {
          this.#listeners.forEach((listener) => {
            listener.tick(tick);
          });
        }
      },
      dropped: (closed) => {
        if (current) {
          this.#dropped(closed);
        }
      },
    };
    let close: () => void;
    try {
      close = this.#open(names, handlers);
    } catch {
      // A transport that refuses to open (the in-browser host throws for a name it cannot
      // watch) is a closed stream: tried again after the wait, never an uncaught error.
      close = () => undefined;
      this.#close = close;
      this.#dropped(true);
      return;
    }
    this.#close = () => {
      current = false;
      close();
    };
  }

  #opened(): void {
    this.#delay = REOPEN_DELAY_FIRST_MS;
    this.#setStatus("live");
    this.#listeners.forEach((listener) => {
      listener.opened();
    });
  }

  #dropped(closed: boolean): void {
    this.#setStatus("reconnecting");
    if (!closed) {
      return;
    }
    const delay = this.#delay;
    this.#delay = Math.min(this.#delay * 2, REOPEN_DELAY_MAX_MS);
    this.#reopenTimer = this.#timers.set(() => {
      this.#reopenTimer = undefined;
      this.#restart(true);
    }, delay);
  }

  #setStatus(status: StreamStatus): void {
    if (status !== this.status) {
      this.status = status;
      this.#statusListeners.forEach((listener) => {
        listener();
      });
    }
  }
}

/** The part of the browser's EventSource the stream uses. */
export interface EventSourceLike {
  readonly readyState: number;
  onopen: ((event: Event) => void) | null;
  onerror: ((event: Event) => void) | null;
  addEventListener(type: "tick", listener: (event: MessageEvent<string>) => void): void;
  close(): void;
}

/** EventSource's readyState once it gave up (it will not reconnect by itself). */
const EVENT_SOURCE_CLOSED = 2;

/**
 * The API's revision stream (`GET /events/stream?domain=...`, H6) over Server-Sent Events,
 * at `base` (the server's origin, or "" for the page's). The browser reconnects a dropped
 * stream by itself, firing `open` again; one it gave up on is reported closed.
 */
export function eventSourceTicks(
  base: string,
  create: (url: string) => EventSourceLike = (url) => new EventSource(url, { withCredentials: true }),
): OpenTicks {
  return (watching, handlers) => {
    const query = watching.map((name) => `domain=${encodeURIComponent(name)}`).join("&");
    const source = create(`${base}/events/stream?${query}`);
    source.onopen = () => {
      handlers.opened();
    };
    source.addEventListener("tick", (event) => {
      handlers.tick(JSON.parse(event.data) as Tick);
    });
    source.onerror = () => {
      const closed = source.readyState === EVENT_SOURCE_CLOSED;
      if (closed) {
        source.close();
      }
      handlers.dropped(closed);
    };
    return () => {
      source.close();
    };
  };
}
