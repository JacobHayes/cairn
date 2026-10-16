// A13, A11: importing a route file reads it, parses it and delivers it as an import write; where
// a draft is open the import is refused and the person may discard that draft and import. Both
// writes are the edit manager's, found by the flow's edit keys.
import type { Answered, HttpFailure } from "@cairn/client";
import { createActor } from "xstate";
import { describe, expect, it } from "vitest";

import { startRoot } from "../data/edits.test-support.ts";
import { deferred, settled } from "../data/fake.test-support.ts";
import type { Route, RouteFile } from "../data/host.ts";
import { RESEND_COUNT_MAX } from "../data/write-machine.ts";
import { importFlow, type Imported } from "./import.ts";

const keys = { import: "import:new-route", discard: "import:new-route:discard" };
const file = { format: 1, route: "r_one", name: "One", nodes: [] } as unknown as RouteFile;
const picked = new File(["route: r_one"], "one.yaml");
const receipt = { patch_id: "p_x", domain: { route: "r_one" }, content_hash: "h", revision: 5 };
const applied: Answered<HttpFailure> = { outcome: "answered", answer: { outcome: "applied", receipt, consequences: {} } };
const refused: Answered<HttpFailure> = { outcome: "rejected", rejection: { rejection: "invalid", violations: [{ at: {}, code: "draft_exists", message: "open" }] } };
const lost: Answered<HttpFailure> = { outcome: "failed", error: { status: 0, message: "down" } };
const open = { id: "r_one", revision: 5, draft: { revision: 2 } } as unknown as Route;
const closed = { id: "r_one", revision: 5, draft: null } as unknown as Route;

interface Options {
  read?: (text: string) => RouteFile;
  route?: (id: string) => Promise<Route>;
}

async function started(options: Options = {}) {
  const root = await startRoot();
  const routes: string[] = [];
  const flow = createActor(importFlow, {
    input: {
      edits: root.edits,
      keys,
      read: options.read ?? (() => file),
      route: (id) => {
        routes.push(id);
        return (options.route ?? (() => Promise.resolve(open)))(id);
      },
    },
  });
  const imported: Imported[] = [];
  flow.on("imported", (each) => imported.push(each));
  flow.start();
  return { ...root, flow, imported, routes };
}

const editsOf = (writes: ReturnType<Awaited<ReturnType<typeof started>>["writes"]>) => Object.values(writes).map((entry) => entry.snapshot.context.about.edit);

describe("choosing a file", () => {
  it("delivers it as an import write and tells whoever started the flow when it landed", async () => {
    const { flow, host, imported } = await started();
    host.imports = [applied];
    flow.send({ type: "choose", file: picked });
    await settled();
    expect(host.imported).toHaveLength(1);
    expect(host.imported[0]).toMatchObject({ file: { route: "r_one" } });
    expect(imported).toEqual([{ type: "imported", file, answer: expect.objectContaining({ outcome: "applied" }) as unknown }]);
    expect(flow.getSnapshot().value).toBe("idle");
  });

  it("submits nothing for a file that does not parse, and says why", async () => {
    const { flow, host } = await started({
      read: () => {
        throw new Error("line 3: not a route file");
      },
    });
    flow.send({ type: "choose", file: picked });
    await settled();
    expect(host.imported).toEqual([]);
    expect(flow.getSnapshot().value).toBe("idle");
    expect(flow.getSnapshot().context.unreadable).toContain("line 3: not a route file");
  });

  it("clears what it said of a bad file when the next one is chosen", async () => {
    let good = false;
    const { flow, host } = await started({
      read: () => {
        if (!good) {
          throw new Error("bad");
        }
        return file;
      },
    });
    flow.send({ type: "choose", file: picked });
    await settled();
    good = true;
    host.imports = [applied];
    flow.send({ type: "choose", file: picked });
    await settled();
    expect(flow.getSnapshot().context.unreadable).toBeUndefined();
    expect(host.imported).toHaveLength(1);
  });

  it("keeps a refusal as the import write's, and the flow is ready again", async () => {
    const { flow, host, imported, writes } = await started();
    host.imports = [refused];
    flow.send({ type: "choose", file: picked });
    await settled();
    expect(imported).toEqual([]);
    expect(editsOf(writes())).toEqual([keys.import]);
    expect(Object.values(writes())[0]?.snapshot.context.command).toMatchObject({ kind: "import", request: { file: { route: "r_one" } } });
    expect(flow.getSnapshot().value).toBe("idle");
  });

  it("takes no second start while it works", async () => {
    const held = deferred<Answered<HttpFailure>>();
    const { flow, host } = await started();
    host.imports = [held.promise, applied];
    flow.send({ type: "choose", file: picked });
    await settled();
    flow.send({ type: "choose", file: picked });
    flow.send({ type: "replace", file });
    held.resolve(applied);
    await settled();
    expect(host.imported).toHaveLength(1);
  });
});

describe("replacing the open draft", () => {
  it("discards it at the revision the route is read at, then imports the file", async () => {
    const { flow, host, imported, routes } = await started();
    host.answers = [applied];
    host.imports = [applied];
    flow.send({ type: "replace", file });
    await settled();
    expect(routes).toEqual(["r_one"]);
    expect(host.sent).toHaveLength(1);
    expect(host.sent[0]).toMatchObject({ target: { route: "r_one" }, base_revision: 5, mutations: [{ op: "discard_draft" }] });
    expect(host.imported).toHaveLength(1);
    expect(imported).toHaveLength(1);
  });

  it("skips the discard where no draft is open now", async () => {
    const { flow, host } = await started({ route: () => Promise.resolve(closed) });
    host.imports = [applied];
    flow.send({ type: "replace", file });
    await settled();
    expect(host.sent).toEqual([]);
    expect(host.imported).toHaveLength(1);
  });

  it("imports nothing, and keeps the discard's rejection under its own key, when the discard is refused", async () => {
    const { flow, host, writes } = await started();
    host.answers = [{ outcome: "rejected", rejection: { rejection: "invalid", violations: [] } }];
    flow.send({ type: "replace", file });
    await settled();
    expect(host.imported).toEqual([]);
    expect(editsOf(writes())).toEqual([keys.discard]);
    expect(flow.getSnapshot().value).toBe("idle");
  });

  it("imports nothing, and is ready again, when the route cannot be read", async () => {
    const { flow, host } = await started({ route: () => Promise.reject(new Error("no such route")) });
    flow.send({ type: "replace", file });
    await settled();
    expect(host.sent).toEqual([]);
    expect(host.imported).toEqual([]);
    expect(flow.getSnapshot().context.unreadable).toContain("no such route");
    expect(flow.getSnapshot().value).toBe("idle");
  });

  it("is left idle by a discard that is waiting for Retry, and a replace after the Retry lands it imports without a second discard", async () => {
    let current = open;
    const { flow, host, edits, advance, writes } = await started({ route: () => Promise.resolve(current) });
    host.answers = Array.from({ length: RESEND_COUNT_MAX + 1 }, () => lost);
    flow.send({ type: "replace", file });
    await settled();
    for (let resend = 0; resend < RESEND_COUNT_MAX; resend += 1) {
      await advance(15_000);
    }
    expect(flow.getSnapshot().value).toBe("idle");
    expect(host.imported).toEqual([]);
    // The person retries by hand from the chip, and the discard lands.
    host.answers = [applied];
    edits.send({ type: "retry", id: Object.keys(writes())[0] ?? "" });
    await settled();
    expect(writes()).toEqual({});
    current = closed;
    const discards = host.sent.length;
    host.imports = [applied];
    flow.send({ type: "replace", file });
    await settled();
    expect(host.imported).toHaveLength(1);
    expect(host.sent).toHaveLength(discards);
  });
});

describe("stopping the flow", () => {
  it("leaves the import write in the manager to land and be saved", async () => {
    const held = deferred<Answered<HttpFailure>>();
    const { flow, host, saves, imported } = await started();
    host.imports = [held.promise];
    flow.send({ type: "choose", file: picked });
    await settled();
    flow.stop();
    held.resolve(applied);
    await settled();
    expect(saves).toHaveLength(1);
    expect(imported).toEqual([]);
  });
});
