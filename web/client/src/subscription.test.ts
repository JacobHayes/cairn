// H6: the page's tick stream reopens when what its views watch changes and after a drop,
// and every reopening tells the views, whose current revisions then ask again.
import { SimulatedClock, createActor, fromCallback, type AnyActorRef } from "xstate";
import { describe, expect, it } from "vitest";

import {
  backoffDelayMs,
  REOPEN_DELAY_FIRST_MS,
  REOPEN_DELAY_MAX_MS,
  eventSourceTicks,
  streamStatus,
  subscription,
  type EventSourceLike,
  type OpenTicks,
  type StreamEvent,
  type TickHandlers,
} from "./subscription.ts";
import { UNTRACKED, fetched, reopened, ticked, type Tick, type Tracked } from "./tracker.ts";

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

/** A view that hears the stream: an actor that records what it is sent. */
function viewer() {
  const heard: StreamEvent[] = [];
  const ref: AnyActorRef = createActor(
    fromCallback<StreamEvent>(({ receive }) => {
      receive((event) => heard.push(event));
    }),
  ).start();
  return { ref, heard };
}

/** The subscription over a transport, on a clock the test advances. */
function started(open: OpenTicks) {
  const clock = new SimulatedClock();
  const actor = createActor(subscription, { clock, input: { open } }).start();
  const watch = (id: string, names: string[], to: AnyActorRef = viewer().ref) => {
    actor.send({ type: "watch", id, names, to });
  };
  /** Runs what a batch of changes raised for itself. */
  const flush = () => {
    clock.increment(0);
  };
  const status = () => streamStatus(actor.getSnapshot());
  return { actor, clock, watch, flush, status };
}

const journeyTick = (revision: number): Tick => ({ of: { domain: { journey: "j_one" } }, revision });

describe("subscription", () => {
  it("opens one stream for a batch of watches, and reopens when the union changes", () => {
    const { opened, open } = transport();
    const { actor, watch, flush } = started(open);
    watch("a", ["journey:j_one", "deployment"]);
    watch("b", ["deployment"]);
    flush();
    expect(opened.map((stream) => stream.watching)).toEqual([["deployment", "journey:j_one"]]);
    actor.send({ type: "unwatch", id: "a" });
    flush();
    expect(opened.map((stream) => stream.watching)).toEqual([["deployment", "journey:j_one"], ["deployment"]]);
    expect(opened[0]?.closed).toBe(true);
  });

  it("opens no second stream for a watch the stream already covers, and closes with the last one", () => {
    const { opened, open, latest } = transport();
    const { actor, watch, flush, status } = started(open);
    watch("a", ["deployment"]);
    flush();
    watch("b", ["deployment"]);
    flush();
    expect(opened).toHaveLength(1);
    actor.send({ type: "unwatch", id: "a" });
    actor.send({ type: "unwatch", id: "b" });
    flush();
    expect(latest().closed).toBe(true);
    expect(status()).toBe("idle");
  });

  it("tells every watcher when it reopens, so a reconnect's current revisions ask again", () => {
    const { open, latest } = transport();
    const { watch, flush, status } = started(open);
    const first = viewer();
    const second = viewer();
    watch("first", ["journey:j_one"], first.ref);
    watch("second", ["journey:j_one"], second.ref);
    flush();
    expect(status()).toBe("connecting");
    // A view's own tracking: it forgets what it asked for when the stream opens, and asks for a newer tick.
    let tracked: Tracked = fetched(UNTRACKED, journeyTick(0).of, 4);
    const refetched: number[] = [];
    const hear = (event: StreamEvent) => {
      if (event.type === "stream.opened") {
        tracked = reopened(tracked);
      } else {
        const next = ticked(tracked, event.tick);
        if (next !== undefined) {
          tracked = next;
          refetched.push(event.tick.revision);
        }
      }
    };
    latest().handlers.opened();
    latest().handlers.tick(journeyTick(4));
    latest().handlers.tick(journeyTick(5));
    first.heard.forEach(hear);
    expect(refetched).toEqual([5]);
    expect(second.heard).toEqual(first.heard);
    expect(status()).toBe("live");
    latest().handlers.dropped(false);
    expect(status()).toBe("reconnecting");
    latest().handlers.opened();
    latest().handlers.tick(journeyTick(5));
    first.heard.slice(3).forEach(hear);
    expect(refetched).toEqual([5, 5]);
    expect(status()).toBe("live");
  });
});

describe("subscription after a drop", () => {
  it("backs off from the first wait, doubling, to the longest, whatever the attempt count", () => {
    expect([0, 1, 2, 3].map(backoffDelayMs)).toEqual([REOPEN_DELAY_FIRST_MS, 2 * REOPEN_DELAY_FIRST_MS, 4 * REOPEN_DELAY_FIRST_MS, 8 * REOPEN_DELAY_FIRST_MS]);
    expect(backoffDelayMs(6)).toBe(REOPEN_DELAY_MAX_MS);
    expect(backoffDelayMs(10_000)).toBe(REOPEN_DELAY_MAX_MS);
  });

  it("reopens a closed stream after a wait that doubles up to the bound, and starts over once one opens", () => {
    const { opened, open, latest } = transport();
    const { clock, watch, flush, status } = started(open);
    watch("a", ["deployment"]);
    flush();
    for (let drop = 0; drop < 8; drop += 1) {
      const wait = backoffDelayMs(drop);
      const before = opened.length;
      latest().handlers.dropped(true);
      expect(status()).toBe("reconnecting");
      clock.increment(wait - 1);
      expect(opened).toHaveLength(before);
      clock.increment(1);
      expect(opened).toHaveLength(before + 1);
    }
    expect(backoffDelayMs(7)).toBe(REOPEN_DELAY_MAX_MS);
    latest().handlers.opened();
    const before = opened.length;
    latest().handlers.dropped(true);
    clock.increment(REOPEN_DELAY_FIRST_MS);
    expect(opened).toHaveLength(before + 1);
  });

  it("reopens at once on request, and does not wait out a closed stream's delay", () => {
    const { opened, open, latest } = transport();
    const { actor, clock, watch, flush } = started(open);
    watch("a", ["deployment"]);
    flush();
    latest().handlers.dropped(true);
    actor.send({ type: "reopen" });
    expect(opened).toHaveLength(2);
    // The wait that was cancelled opens nothing more.
    clock.increment(REOPEN_DELAY_MAX_MS);
    expect(opened).toHaveLength(2);
  });

  it("ignores a stream it has closed", () => {
    const { open, latest } = transport();
    const { actor, watch, flush } = started(open);
    const view = viewer();
    watch("a", ["journey:j_one"], view.ref);
    flush();
    const first = latest();
    actor.send({ type: "reopen" });
    first.handlers.tick(journeyTick(9));
    latest().handlers.tick(journeyTick(10));
    expect(view.heard).toEqual([{ type: "stream.tick", tick: journeyTick(10) }]);
  });
});

describe("subscription over a transport that refuses", () => {
  it("treats a stream that throws on opening as closed, and opens it again after the wait", () => {
    let attempts = 0;
    const { clock, watch, flush, status } = started((_watching, handlers) => {
      attempts += 1;
      if (attempts === 1) {
        throw new Error("cannot watch that");
      }
      handlers.opened();
      return () => undefined;
    });
    watch("a", ["deployment"]);
    flush();
    expect(status()).toBe("reconnecting");
    clock.increment(REOPEN_DELAY_FIRST_MS);
    expect(attempts).toBe(2);
    expect(status()).toBe("live");
  });
});

describe("a stopped subscription", () => {
  it("closes the transport and leaves nothing to fire", () => {
    const { opened, open, latest } = transport();
    const { actor, clock, watch, flush } = started(open);
    const view = viewer();
    watch("a", ["deployment"], view.ref);
    flush();
    const stream = latest();
    stream.handlers.dropped(true);
    actor.stop();
    expect(stream.closed).toBe(true);
    clock.increment(REOPEN_DELAY_MAX_MS);
    stream.handlers.tick(journeyTick(1));
    expect(opened).toHaveLength(1);
    expect(view.heard).toEqual([]);
  });

  it("closes a live stream", () => {
    const { open, latest } = transport();
    const { actor, watch, flush } = started(open);
    watch("a", ["deployment"]);
    flush();
    latest().handlers.opened();
    actor.stop();
    expect(latest().closed).toBe(true);
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
