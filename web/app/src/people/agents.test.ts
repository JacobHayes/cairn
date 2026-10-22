// What the Connect an agent page shows for each shape of sign-in, and the exact commands a client
// parses (their arguments, not the prose around them).
import { describe, expect, it } from "vitest";

import type { McpAccess } from "../data/host.ts";
import { agentSetup, shellLine, tokenCommands } from "./agents.ts";

const URL = "https://cairn.example.com/api/mcp";
const access = (sign_in: McpAccess["sign_in"], tokens = false): McpAccess => ({ url: URL, sign_in, tokens });

describe("agentSetup", () => {
  it.each([
    ["built-in sign-in", access(["oauth"], true), { closed: false, browser: true, login: true, tokens: true }],
    ["the relay to Google", access(["google_oauth"]), { closed: false, browser: true, login: true, tokens: false }],
    ["a tailnet", access(["tailnet"]), { closed: false, browser: false, login: false, tokens: false }],
    ["a dev user", access(["dev_user"]), { closed: false, browser: false, login: false, tokens: false }],
    ["no way in", access([]), { closed: true, browser: false, login: false, tokens: false }],
  ] as const)("for %s says whether there is a browser step, a login command and tokens", (_, mcp, expected) => {
    const setup = agentSetup(mcp, "claude");
    expect({ closed: setup.closed, browser: setup.browser, login: setup.login !== undefined, tokens: setup.tokens }).toEqual(expected);
  });

  it("marks a way in that answers only what another did not", () => {
    const notes = agentSetup(access(["tailnet", "oauth"], true), "claude").notes;
    expect(notes.map((note) => note.otherwise)).toEqual([false, true]);
    // A dev user beside another way in answers only what that one did not, unless it is asked first.
    expect(agentSetup(access(["oauth", "dev_user"], true), "claude").notes.map((note) => note.otherwise)).toEqual([false, true]);
    expect(agentSetup(access(["dev_user", "oauth"], true), "claude").notes.map((note) => note.otherwise)).toEqual([false, false]);
    expect(agentSetup(access(["dev_user"]), "claude").notes.map((note) => note.otherwise)).toEqual([false]);
  });

  it("promises the agent is recorded beside the person only where no provider asked first would name the person alone", () => {
    const marked = (sign_in: McpAccess["sign_in"]) => agentSetup(access(sign_in, true), "claude").notes.find((note) => note.kind === "oauth")?.marked;
    expect(marked(["oauth"])).toBe(true);
    expect(marked(["oauth", "dev_user"])).toBe(true);
    expect(marked(["dev_user", "oauth"])).toBe(false);
    expect(marked(["tailnet", "oauth"])).toBe(false);
  });

  it("gives each client its own command with the server's address, and Codex no separate login", () => {
    const mcp = access(["oauth"], true);
    expect(agentSetup(mcp, "claude").add).toEqual(["claude", "mcp", "add", "--transport", "http", "--scope", "user", "cairn", URL]);
    expect(agentSetup(mcp, "claude").login).toEqual(["claude", "mcp", "login", "cairn"]);
    expect(agentSetup(mcp, "codex").add).toEqual(["codex", "mcp", "add", "cairn", "--url", URL]);
    expect(agentSetup(mcp, "codex").login).toBeUndefined();
    expect(agentSetup(mcp, "other").add).toBeUndefined();
  });
});

describe("tokenCommands", () => {
  const commands = tokenCommands(access(["oauth"], true), "cairn_agent_secret");

  it("removes a Claude entry that exists before adding it again with the token, as a second add of the name is refused", () => {
    expect(commands.claudeRemove).toEqual(["claude", "mcp", "remove", "--scope", "user", "cairn"]);
  });

  it("puts Claude's variadic header after the name and the address, or it would take them", () => {
    const at = (arg: string) => commands.claude.indexOf(arg);
    expect(at("--header")).toBeGreaterThan(at(URL));
    expect(at(URL)).toBeGreaterThan(at("cairn"));
    expect(commands.claude.at(-1)).toBe("Authorization: Bearer cairn_agent_secret");
  });

  it("gives Codex the token through an environment variable it names, not on the command line", () => {
    expect(commands.codexVariable).toEqual(["export", "CAIRN_TOKEN=cairn_agent_secret"]);
    expect(commands.codex).toEqual(["codex", "mcp", "add", "cairn", "--url", URL, "--bearer-token-env-var", "CAIRN_TOKEN"]);
    expect(commands.codex.join(" ")).not.toContain("cairn_agent_secret");
  });
});

describe("shellLine", () => {
  it("quotes only an argument a shell would split or expand", () => {
    expect(shellLine(["claude", "mcp", "add", URL, "Authorization: Bearer abc", 'a"b$c'])).toBe(`claude mcp add ${URL} "Authorization: Bearer abc" "a\\"b\\$c"`);
  });
});
