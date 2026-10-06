// The host switch.
import { describe, expect, it } from "vitest";

import { chooseHost, requestedHost, switchedTo } from "./choose.ts";

describe("the host switch", () => {
  it.each([
    ["?host=server", "server"],
    ["?host=browser", "browser"],
    ["?host=elsewhere", undefined],
    ["", undefined],
  ])("reads %j as %s", (search, kind) => {
    expect(requestedHost(search)).toBe(kind);
  });

  it("asks for a server only when the address does not choose", async () => {
    expect(await chooseHost("?host=browser", () => Promise.resolve(true))).toBe("browser");
    expect(await chooseHost("", () => Promise.resolve(true))).toBe("server");
    expect(await chooseHost("", () => Promise.resolve(false))).toBe("browser");
  });

  it("switches keeping the screen and the rest of the address", () => {
    const at = { origin: "http://cairn.test", pathname: "/", search: "?host=server&x=1", hash: "#/journeys/j_one" };
    expect(switchedTo(at, "browser")).toBe("http://cairn.test/?host=browser&x=1#/journeys/j_one");
  });
});
