// C16: the journey index's query as `GET /api/journeys` reads it: repeated statuses and identities,
// and nothing for a filter left empty. A domain document is read only under the build the page
// was loaded from (ARCHITECTURE, Web UI: version skew).
import { afterEach, describe, expect, it, vi } from "vitest";


import { Skewed } from "./host.ts";
import { indexParams, serverHost } from "./server-host.ts";

describe("the index's query parameters", () => {
  it("names each filter given and leaves out the empty ones", () => {
    expect(indexParams({ status: ["active"], route: "r", version: 2, referencing: ["who_a", "who_b"], upgrade_available: false, after: "j_x" })).toEqual({
      status: ["active"],
      route: "r",
      version: 2,
      referencing: ["who_a", "who_b"],
      upgrade_available: false,
      after: "j_x",
    });
    expect(indexParams({ status: [], referencing: [] })).toEqual({});
    expect(indexParams()).toEqual({});
  });
});

describe("a journey's document", () => {
  const digest = "ab".repeat(32);

  /** The server host of a page loaded from `build`, over a server that stamps every answer with `headers`. */
  function stampedWith(headers: Record<string, string>, build: string | undefined) {
    vi.stubGlobal("fetch", () => Promise.resolve(new Response("{}", { headers })));
    return serverHost("http://cairn.test", build);
  }

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("is read under the page's build, and refused as skew under another or none; a page with none (Vite's dev server) compares nothing", async () => {
    await expect(stampedWith({ "cairn-build": digest }, digest).documentText("j_one")).resolves.toBe("{}");
    await expect(stampedWith({ "cairn-build": "cd".repeat(32) }, digest).documentText("j_one")).rejects.toBeInstanceOf(Skewed);
    await expect(stampedWith({}, digest).documentText("j_one")).rejects.toBeInstanceOf(Skewed);
    await expect(stampedWith({}, undefined).documentText("j_one")).resolves.toBe("{}");
  });
});
