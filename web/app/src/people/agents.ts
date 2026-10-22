// I2: what the Connect an agent page shows for one deployment, worked out from the capabilities'
// MCP access alone. The page never reads provider kinds or builds an address of its own: the
// server says the address and how an agent signs in, and this module decides which paragraphs
// and which commands follow, so the component only renders them.
import type { McpAccess } from "../data/host.ts";

/** The clients the page has instructions for. */
export type Client = "claude" | "codex" | "other";

export const CLIENTS: readonly { value: Client; label: string; title: string }[] = [
  { value: "claude", label: "Claude Code", title: "Show the steps for Claude Code" },
  { value: "codex", label: "Codex", title: "Show the steps for Codex" },
  { value: "other", label: "Other agent", title: "Show the address and what any other MCP client needs" },
];

type SignIn = McpAccess["sign_in"][number];

/**
 * One sign-in paragraph under the steps. `otherwise`: it answers only what another way in did not.
 * `marked`: an agent signed in this way is recorded beside the person, which holds only when no
 * provider asked before it would name the person alone (the first with a credential names the actor).
 */
export interface SignInNote {
  kind: SignIn;
  otherwise: boolean;
  marked: boolean;
}

/** What the page shows for the client picked. */
export interface AgentSetup {
  /** No way for an agent to sign in: the page says so and shows nothing else. */
  closed: boolean;
  notes: SignInNote[];
  /** Whether the first use is a browser sign-in: only then are there login steps. */
  browser: boolean;
  /** The command that adds Cairn to the client, as arguments; none for a client with no command. */
  add: string[] | undefined;
  /** The command that signs in, as arguments; none where there is no browser sign-in or no command. */
  login: string[] | undefined;
  /** Whether the person can make and revoke agent tokens here. */
  tokens: boolean;
}

/** The server's name for the MCP server in the client's list. */
const SERVER = "cairn";

/** I2: the setup for `client`, from what this deployment says an agent needs. */
export function agentSetup(mcp: McpAccess, client: Client): AgentSetup {
  const browser = mcp.sign_in.some((kind) => kind === "oauth" || kind === "google_oauth");
  const tailnet = mcp.sign_in.includes("tailnet");
  const notes = mcp.sign_in.map((kind, at) => ({
    kind,
    // A browser sign-in beside a tailnet answers the devices the tailnet did not; a dev user anywhere but first, what nothing before it took.
    otherwise: kind === "dev_user" ? at > 0 : tailnet && (kind === "oauth" || kind === "google_oauth"),
    marked: kind === "oauth" && !mcp.sign_in.slice(0, at).some((before) => before === "tailnet" || before === "dev_user"),
  }));
  const add = client === "claude" ? ["claude", "mcp", "add", "--transport", "http", "--scope", "user", SERVER, mcp.url] : client === "codex" ? ["codex", "mcp", "add", SERVER, "--url", mcp.url] : undefined;
  return {
    closed: mcp.sign_in.length === 0,
    notes,
    browser,
    add,
    // Codex signs in as it adds the server; `codex mcp login` is only for signing in again.
    login: client === "claude" && browser ? ["claude", "mcp", "login", SERVER] : undefined,
    tokens: mcp.tokens,
  };
}

/** The environment variable Codex reads a token from. */
export const TOKEN_VARIABLE = "CAIRN_TOKEN";

/** What a new token is used with, per client: the commands that carry it. */
export interface TokenCommands {
  /** Claude refuses to add a server that exists, so a person who added Cairn before removes it first. */
  claudeRemove: string[];
  claude: string[];
  /** Codex reads the token from an environment variable, which the person sets where it runs. */
  codexVariable: string[];
  codex: string[];
}

/** I2: the commands that give each client `token` as a bearer. Claude's `--header` is variadic, so it goes after the name and the address. */
export function tokenCommands(mcp: McpAccess, token: string): TokenCommands {
  return {
    claudeRemove: ["claude", "mcp", "remove", "--scope", "user", SERVER],
    claude: ["claude", "mcp", "add", "--transport", "http", "--scope", "user", SERVER, mcp.url, "--header", `Authorization: Bearer ${token}`],
    codexVariable: ["export", `${TOKEN_VARIABLE}=${token}`],
    codex: ["codex", "mcp", "add", SERVER, "--url", mcp.url, "--bearer-token-env-var", TOKEN_VARIABLE],
  };
}

/** A command line for a shell: an argument with anything outside the plain set is double-quoted. */
export function shellLine(args: readonly string[]): string {
  return args.map((arg) => (/^[A-Za-z0-9_@%+=:,./-]+$/.test(arg) ? arg : `"${arg.replace(/(["\\$`])/g, "\\$1")}"`)).join(" ");
}
