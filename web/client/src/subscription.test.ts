// H6: the page's tick stream reopens when what its views watch changes and after a drop,
// and every reopening tells the views, whose current revisions then ask again.
import { describe, expect, it } from "vitest";

import {
  REOPEN_DELAY_FIRST_MS,
  REOPEN_DELAY_MAX_MS,
  Subscription,
  eventSourceTicks,
  type EventSourceLike,
  type OpenTicks,
  type TickHandlers,
  type Timers,
} from "./subscription.ts";
import { Tracker, type Tick } from "./tracker.ts";

/** A transport that records each stream opened and lets the test drive the latest. */
function transport() {
  const opened: { watching: string[]; handlers: TickHandlers; closed: boolean }[] = [];
  const open: OpenTicks = (watching, handlers) => {
    const stream = { watching: [...watching], handlers, closed: false };
    opened.push(stream);
    return () => {
      stream.closed = true;
    };
  };
  const latest = () => {
    const stream = opened.at(-1);
    if (stream === undefined) {
      throw new Error("no stream was opened");
    }
    return stream;
  };
  return { opened, open, latest };
}

/** Timers the test fires by hand. */
function manualTimers() {
  const pending: { callback: () => void; ms: number }[] = [];
  const timers: Timers = {
    set: (callback, ms) => pending.push({ callback, ms }),
    clear: () => undefined,
  };
  const fire = () => {
    const next = pending.shift();
    next?.callback();
    return next?.ms;
  };
  return { timers, fire };
}

const settled = () =>
  new Promise<void>((resolve) => {
    queueMicrotask(resolve);
  });
const journeyTick = (revision: number): Tick => ({ of: { domain: { journey: "j_one" } }, revision });

describe("Subscription", () => {
  it("opens one stream for a batch of watches, and reopens when the union changes", async () => {
    const { opened, open } = transport();
    const subscription = new Subscription(open);
    const releaseJourney = subscription.watch(["journey:j_one", "deployment"]);
    subscription.watch(["deployment"]);
    await settled();
    expect(opened.map((stream) => stream.watching)).toEqual([["deployment", "journey:j_one"]]);
    releaseJourney();
    await settled();
    expect(opened.map((stream) => stream.watching)).toEqual([["deployment", "journey:j_one"], ["deployment"]]);
    expect(opened[0]?.closed).toBe(true);
  });

  it("tells views when it reopens, so a reconnect's current revisions ask again", async () => {
    const { open, latest } = transport();
    const subscription = new Subscription(open);
    const tracker = new Tracker();
    tracker.fetched(journeyTick(0).of, 4);
    const refetched: number[] = [];
    subscription.listen({
      opened: () => {
        tracker.reopened();
      },
      tick: (tick) => {
        if (tracker.ticked(tick)) {
          refetched.push(tick.revision);
        }
      },
    });
    subscription.watch(["journey:j_one"]);
    await settled();
    latest().handlers.opened();
    latest().handlers.tick(journeyTick(4));
    latest().handlers.tick(journeyTick(5));
    expect(refetched).toEqual([5]);
    latest().handlers.dropped(false);
    expect(subscription.status).toBe("reconnecting");
    latest().handlers.opened();
    latest().handlers.tick(journeyTick(5));
    expect(refetched).toEqual([5, 5]);
    expect(subscription.status).toBe("live");
  });

});

describe("Subscription after a drop", () => {
  it("reopens a closed stream after a wait that doubles up to the bound", async () => {
    const { opened, open, latest } = transport();
    const { timers, fire } = manualTimers();
    const subscription = new Subscription(open, timers);
    subscription.watch(["deployment"]);
    await settled();
    const waits: (number | undefined)[] = [];
    for (let drop = 0; drop < 8; drop += 1) {
      latest().handlers.dropped(true);
      waits.push(fire());
    }
    expect(waits[0]).toBe(REOPEN_DELAY_FIRST_MS);
    expect(waits.every((wait, at) => at === 0 || (wait ?? 0) >= (waits[at - 1] ?? 0))).toBe(true);
    expect(Math.max(...waits.map((wait) => wait ?? 0))).toBe(REOPEN_DELAY_MAX_MS);
    expect(opened).toHaveLength(9);
    latest().handlers.opened();
    latest().handlers.dropped(true);
    expect(fire()).toBe(REOPEN_DELAY_FIRST_MS);
  });

  it("ignores a stream it has closed", async () => {
    const { open, latest } = transport();
    const subscription = new Subscription(open);
    const heard: number[] = [];
    subscription.listen({ opened: () => undefined, tick: (tick) => heard.push(tick.revision) });
    subscription.watch(["journey:j_one"]);
    await settled();
    const first = latest();
    subscription.reopen();
    first.handlers.tick(journeyTick(9));
    latest().handlers.tick(journeyTick(10));
    expect(heard).toEqual([10]);
  });
});

describe("Subscription over a transport that refuses", () => {
  it("treats a stream that throws on opening as closed, and opens it again after the wait", async () => {
    const { timers, fire } = manualTimers();
    let attempts = 0;
    const subscription = new Subscription((_watching, handlers) => {
      attempts += 1;
      if (attempts === 1) {
        throw new Error("cannot watch that");
      }
      handlers.opened();
      return () => undefined;
    }, timers);
    subscription.watch(["deployment"]);
    await settled();
    expect(subscription.status).toBe("reconnecting");
    expect(fire()).toBe(REOPEN_DELAY_FIRST_MS);
    expect(attempts).toBe(2);
    expect(subscription.status).toBe("live");
  });
});

describe("eventSourceTicks", () => {
  it("watches each name, reads tick events, and reports a source that gave up as closed", () => {
    const listeners: ((event: MessageEvent<string>) => void)[] = [];
    let url = "";
    const source: EventSourceLike & { readyState: number } = {
      readyState: 0,
      onopen: null,
      onerror: null,
      addEventListener: (_type, listener) => listeners.push(listener),
      close: () => undefined,
    };
    const heard: string[] = [];
    const close = eventSourceTicks("http://cairn.test", (at) => {
      url = at;
      return source;
    })(["deployment", "journey:j_one"], {
      opened: () => heard.push("opened"),
      tick: (tick) => heard.push(`tick ${String(tick.revision)}`),
      dropped: (closed) => heard.push(`dropped ${String(closed)}`),
    });
    expect(new URL(url).searchParams.getAll("domain")).toEqual(["deployment", "journey:j_one"]);
    source.onopen?.(new Event("open"));
    listeners.forEach((listener) => {
      listener(new MessageEvent("tick", { data: JSON.stringify(journeyTick(3)) }));
    });
    source.onerror?.(new Event("error"));
    source.readyState = 2;
    source.onerror?.(new Event("error"));
    close();
    expect(heard).toEqual(["opened", "tick 3", "dropped false", "dropped true"]);
  });
});
