// H6 for the index screens' reads: a read refetches only for a tick about what it shows and
// newer than it holds (or naming something it does not hold), keeps its answer through a
// failed refetch, and shows a missing domain as missing.
import { Subscription } from "@cairn/client";
import { describe, expect, it } from "vitest";

import { FakeHost, manualTimers, settled } from "./fake.test-support.ts";
import { Missing } from "./host.ts";
import { allPages, LiveRead } from "./live.ts";

const route = (id: string) => ({ domain: { route: id } });

function reading(fetch: () => Promise<number>) {
  const host = new FakeHost();
  const subscription = new Subscription(host.openTicks, manualTimers().timers);
  let fetches = 0;
  const read = new LiveRead<number>(subscription, {
    watching: ["route:r"],
    about: (of) => "domain" in of && typeof of.domain === "object" && "route" in of.domain && of.domain.route === "r",
    fetch: () => {
      fetches += 1;
      return fetch();
    },
    holds: (revision) => [{ of: route("r"), revision }],
  });
  return { host, read, fetches: () => fetches };
}

describe("a live read", () => {
  it("refetches for a newer tick about what it shows, and for nothing else", async () => {
    let revision = 3;
    const { host, read, fetches } = reading(() => Promise.resolve(revision));
    read.mount();
    await settled();
    host.open();
    await settled();
    const opened = fetches();
    host.tick({ of: route("r"), revision: 3 });
    host.tick({ of: route("other"), revision: 9 });
    await settled();
    expect(fetches()).toBe(opened);
    revision = 4;
    host.tick({ of: route("r"), revision: 4 });
    await settled();
    expect(read.view).toEqual({ status: "ready", value: 4 });
    expect(fetches()).toBe(opened + 1);
  });

  it("keeps its answer through a failed refetch, and shows a missing domain as missing", async () => {
    let answer: () => Promise<number> = () => Promise.resolve(3);
    const { host, read } = reading(() => answer());
    read.mount();
    await settled();
    answer = () => Promise.reject(new Error("down"));
    host.tick({ of: route("r"), revision: 5 });
    await settled();
    expect(read.view).toEqual({ status: "ready", value: 3 });
    answer = () => Promise.reject(new Missing("r"));
    host.tick({ of: route("r"), revision: 6 });
    await settled();
    expect(read.view.status).toBe("missing");
  });
});

describe("a live read when something may have gone", () => {
  it("asks again when the stream opens again, since a deletion meanwhile is only an absence", async () => {
    const { host, read, fetches } = reading(() => Promise.resolve(3));
    read.mount();
    await settled();
    const before = fetches();
    host.open();
    await settled();
    expect(fetches()).toBe(before + 1);
  });

  it("refetches for a deletion's revision 0, though it is older than what it holds", async () => {
    let revision = 3;
    const { host, read, fetches } = reading(() => Promise.resolve(revision));
    read.mount();
    await settled();
    revision = 0;
    host.tick({ of: route("r"), revision: 0 });
    await settled();
    expect(fetches()).toBe(2);
    expect(read.view).toEqual({ status: "ready", value: 0 });
  });
});

describe("every page", () => {
  it("reads page after page until there is no next", async () => {
    const pages: Record<string, { items: number[]; next?: string }> = { "": { items: [1, 2], next: "b" }, b: { items: [3], next: "c" }, c: { items: [] } };
    const asked: (string | undefined)[] = [];
    const items = await allPages<number, string>((after) => {
      asked.push(after);
      return Promise.resolve(pages[after ?? ""] ?? { items: [] });
    });
    expect(items).toEqual([1, 2, 3]);
    expect(asked).toEqual([undefined, "b", "c"]);
  });
});
