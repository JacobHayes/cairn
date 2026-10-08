// The host switch.
import { describe, expect, it } from "vitest";

import { chooseHost, requestedHost, switchedTo, withoutHost } from "./choose.ts";

describe("the host switch", () => {
  it.each([
    ["?host=server", "server"],
    ["?host=browser", "browser"],
    ["?host=elsewhere", undefined],
    ["", undefined],
  ])("reads %j as %s", (search, kind) => {
    expect(requestedHost(search)).toBe(kind);
  });

  it("asks for a server only when the tab was not asked for a host", async () => {
    expect(await chooseHost("browser", () => Promise.resolve(true))).toBe("browser");
    expect(await chooseHost(undefined, () => Promise.resolve(true))).toBe("server");
    expect(await chooseHost(undefined, () => Promise.resolve(false))).toBe("browser");
  });

  it.each([
    ["?host=browser", ""],
    ["?host=server&status=any", "?status=any"],
    ["?status=any", "?status=any"],
    ["?merge=e_a,e_b&host=browser", "?merge=e_a,e_b"],
    ["", ""],
  ])("leaves the screen's own query of %j as %j", (search, rest) => {
    expect(withoutHost(search)).toBe(rest);
  });

  it("switches keeping the screen and its query", () => {
    const at = { pathname: "/journeys/j_one", search: "?host=server&x=1" };
    expect(switchedTo("http://cairn.test", at, "browser")).toBe("http://cairn.test/journeys/j_one?host=browser&x=1");
  });
});
