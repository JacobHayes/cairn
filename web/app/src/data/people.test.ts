// H3: the names the screens show for user ids: asked together in one read, held, and read again
// when the deployment moves. A name not arrived is "someone", a user the host does not know is
// "someone no longer here", and neither is ever the id.
import { backoffDelayMs } from "@cairn/client";
import { describe, expect, it, vi } from "vitest";

import { HostFailure } from "@cairn/wasm";

import type { Host } from "./host.ts";
import { GONE, PENDING, People } from "./people.ts";

/** A host that names `names` and records each read. */
function hostNaming(names: Record<string, string>) {
  const asked: string[][] = [];
  const host = {
    userNames: (users: readonly string[]) => {
      asked.push([...users]);
      return Promise.resolve(users.flatMap((user) => (names[user] === undefined ? [] : [{ user, name: names[user] }])));
    },
  } as unknown as Host;
  return { host, asked };
}

/** A host whose reads fail with `error` until `recover` makes them name every user "Ana". */
function hostFailing(error: Error) {
  let reads = 0;
  let failing = true;
  const host = {
    userNames: (users: readonly string[]) => {
      reads += 1;
      return failing ? Promise.reject(error) : Promise.resolve(users.map((user) => ({ user, name: "Ana" })));
    },
  } as unknown as Host;
  return { host, reads: () => reads, recover: () => { failing = false; } };
}

const settled = () => new Promise<void>((resolve) => { setTimeout(resolve, 0); });

describe("People", () => {
  it("asks for the users of one moment in one read, and says someone until they are named", async () => {
    const { host, asked } = hostNaming({ u_ana: "Ana Reyes", u_bo: "Bo" });
    const people = new People(host);
    expect([people.name("u_ana", 1), people.name("u_bo", 1), people.name("u_ana", 1)]).toEqual([PENDING, PENDING, PENDING]);
    await settled();
    expect(asked).toEqual([["u_ana", "u_bo"]]);
    expect([people.name("u_ana", 1), people.name("u_bo", 1)]).toEqual(["Ana Reyes", "Bo"]);
    expect(asked).toHaveLength(1);
  });

  it("calls a user the host does not name someone no longer here, and does not ask again", async () => {
    const { host, asked } = hostNaming({});
    const people = new People(host);
    people.name("u_gone", 1);
    await settled();
    expect(people.name("u_gone", 1)).toBe(GONE);
    expect(asked).toHaveLength(1);
  });

  it("reads the held names again when the deployment moves, and tells its listeners when names arrive", async () => {
    const names: Record<string, string> = { u_ana: "Ana" };
    const { host, asked } = hostNaming(names);
    const people = new People(host);
    let told = 0;
    people.subscribe(() => {
      told += 1;
    });
    people.name("u_ana", 1);
    await settled();
    names["u_ana"] = "Ana Reyes";
    expect(people.name("u_ana", 1)).toBe("Ana");
    expect(people.name("u_ana", 2)).toBe("Ana");
    await settled();
    expect(people.name("u_ana", 2)).toBe("Ana Reyes");
    expect(asked).toEqual([["u_ana"], ["u_ana"]]);
    expect(told).toBe(2);
  });

});

describe("People when a read fails", () => {
  it("asks again after a passing failure", async () => {
    vi.useFakeTimers();
    try {
      const failing = hostFailing(new Error("down"));
      const people = new People(failing.host);
      people.name("u_ana", 1);
      await vi.advanceTimersByTimeAsync(0);
      expect(failing.reads()).toBe(1);
      failing.recover();
      await vi.advanceTimersByTimeAsync(backoffDelayMs(0));
      expect(failing.reads()).toBe(2);
      expect(people.name("u_ana", 1)).toBe("Ana");
    } finally {
      vi.useRealTimers();
    }
  });

  it("does not ask again after a refusal until the deployment moves", async () => {
    vi.useFakeTimers();
    try {
      const refusing = hostFailing(new HostFailure({ error: "aborted", message: "the module stopped" }));
      const people = new People(refusing.host);
      people.name("u_bo", 1);
      await vi.advanceTimersByTimeAsync(60_000);
      expect(refusing.reads()).toBe(1);
      people.name("u_bo", 2);
      await vi.advanceTimersByTimeAsync(0);
      expect(refusing.reads()).toBe(2);
    } finally {
      vi.useRealTimers();
    }
  });

});

describe("People when reads overlap", () => {
  it("keeps the newer answer when an older read for the same user lands last", async () => {
    const pending: ((named: { user: string; name: string }[]) => void)[] = [];
    const host = { userNames: () => new Promise((resolve) => pending.push(resolve)) } as unknown as Host;
    const people = new People(host);
    people.name("u_ana", 1);
    await settled();
    people.name("u_ana", 2);
    await settled();
    expect(pending).toHaveLength(2);
    pending[1]?.([{ user: "u_ana", name: "New" }]);
    await settled();
    pending[0]?.([{ user: "u_ana", name: "Old" }]);
    await settled();
    expect(people.name("u_ana", 2)).toBe("New");
  });
});
