// I2: Connect an agent. For the client a person picks, the command that adds this deployment's MCP
// server, worded for the sign-in the deployment has. Where agent tokens are accepted, the same page
// lists the tokens that are not revoked (an agent a person allowed through the browser sign-in is
// one too), revokes them, and makes one for a script or a client that cannot sign in through a
// browser. The server says what an agent needs (the capabilities' MCP access); nothing here reads a
// provider kind or builds an address.
import { useActorRef, useSelector } from "@xstate/react";
import { useEffect, useRef, useState, type ReactNode } from "react";
import type { ActorRefFrom } from "xstate";

import { NodePanel } from "../acting/NodeItem.tsx";
import type { AgentToken, AgentTokenHost, McpAccess } from "../data/host.ts";
import { HOST_READ, valueOf, type ReadSource } from "../data/keyed-read.ts";
import { useCaller, useCapabilities, useKeyedRead, useSession, useToday } from "../data/react.ts";
import { dateWords } from "../status/when.ts";
import { CopyButton } from "../ui/CopyButton.tsx";
import { Failed } from "../ui/Failed.tsx";
import { Button, Input, Segmented } from "../ui/kit.tsx";
import { agentTokens } from "./agent-tokens.ts";
import { CLIENTS, agentSetup, shellLine, tokenCommands, type Client, type SignInNote } from "./agents.ts";
import { Tip } from "../ui/Tip.tsx";

/** The text with a place to break after each slash, so an address wraps between its parts and not inside one. */
function breakable(text: string): ReactNode[] {
  return text.split(/(?<=\/)/).flatMap((part, at) => (at === 0 ? [part] : [<wbr key={at} />, part]));
}

/** A value on the band fill with Copy beside it; `what` names the copy for assistive technology. */
function Copyable({ what, text }: { what: string; text: string }) {
  return (
    <div className="copyable">
      <pre className="mono">{breakable(text)}</pre>
      {/* Keyed by the text, so a copy's "Copied" does not outlive the value it copied. */}
      <CopyButton key={text} what={what} text={text} />
    </div>
  );
}

function Code({ children }: { children: string }) {
  return <code className="mono">{children}</code>;
}

/** What signing in is like, for one way the deployment lets an agent in. */
function SignInParagraph({ note }: { note: SignInNote }) {
  const lead = note.otherwise ? "Otherwise, the first time an agent connects, " : "The first time an agent connects, ";
  switch (note.kind) {
    case "tailnet":
      return <p>No sign-in step: Cairn knows you by your Tailscale login. Run the agent on a machine signed in to your Tailscale network as you; a tagged device, such as a shared server, signs no one in.</p>;
    case "oauth":
      return (
        <p>
          {lead}your browser opens to Cairn so you can allow it. Its changes are recorded as yours{note.marked ? ", marked as made through it," : ""} and you can revoke it under Allowed agents.
        </p>
      );
    case "google_oauth":
      return <p>{lead}your browser opens so you can sign in with your Google account. Hosted connectors, which sign in from their vendor&rsquo;s servers, cannot connect this way.</p>;
    case "dev_user":
      return note.otherwise ? <p>On this development server, an agent that does not sign in acts as its built-in user.</p> : <p>No sign-in step: this development server lets every agent in as its built-in user.</p>;
  }
}

/** The ways to set it up that most people do not need, behind one disclosure. */
function MoreOptions({ client, browser }: { client: Client; browser: boolean }) {
  if (client === "other" || (client === "codex" && !browser)) {
    return null;
  }
  return (
    <details className="stack" data-testid="more-options">
      <summary>{client === "claude" ? (browser ? "No browser on this machine, or project scope" : "Share it in a repository") : "Sign in again, or with no browser"}</summary>
      {client === "claude" ? (
        <>
          {browser ? (
            <p className="muted">
              On a machine with no browser, add <Code>--no-browser</Code> to the login: it prints the address to open elsewhere, and you paste back the address the browser lands on.
            </p>
          ) : null}
          <p className="muted">
            The user scope adds Cairn in every project. To share it with everyone who works in a repository, use <Code>--scope project</Code>, which writes it to the repository&rsquo;s <Code>.mcp.json</Code>.
          </p>
        </>
      ) : (
        <p className="muted">
          Codex opens your browser to sign in as soon as it adds Cairn. To sign in again later, run <Code>codex mcp login cairn</Code>, with <Code>--no-browser</Code> on a machine with no browser.
        </p>
      )}
    </details>
  );
}

function Steps({ mcp, client }: { mcp: McpAccess; client: Client }) {
  const setup = agentSetup(mcp, client);
  const oauth = mcp.sign_in.includes("oauth");
  return (
    <>
      {client === "other" ? (
        <>
          <span className="label">MCP address</span>
          <Copyable what="the address" text={mcp.url} />
          <p>Add it as a remote MCP server over Streamable HTTP.</p>
        </>
      ) : (
        <p>Run this in a terminal on the machine where {client === "claude" ? "Claude Code" : "Codex"} runs:</p>
      )}
      {setup.add === undefined ? null : <Copyable what={`the ${client === "claude" ? "Claude Code" : "Codex"} command`} text={shellLine(setup.add)} />}
      {setup.login === undefined ? null : (
        <>
          <p>Then sign in:</p>
          <Copyable what="the sign-in command" text={shellLine(setup.login)} />
        </>
      )}
      {client === "other" && oauth ? <p>A client that supports MCP&rsquo;s OAuth sign-in finds everything it needs from the address; one that does not can use a token from Allowed agents.</p> : null}
      {setup.notes.map((note) => (
        <SignInParagraph key={note.kind} note={note} />
      ))}
      {client === "other" && oauth ? <p className="muted">Hosted connectors, such as custom connectors in Claude on the web, work only when this address is reachable from the internet.</p> : null}
      <MoreOptions client={client} browser={setup.browser} />
    </>
  );
}

/** The caller's tokens that are not revoked, newest first. */
const TOKENS: ReadSource<{ tokens: AgentTokenHost }, AgentToken[]> = {
  policy: HOST_READ,
  read: ({ tokens }) => tokens.list(),
};

function TokenRow({ token, busy, revoking, onRevoke }: { token: AgentToken; busy: boolean; revoking: boolean; onRevoke: () => void }) {
  const today = useToday();
  return (
    <li className="token-row" data-testid="agent-token">
      <span className="name">{token.name}</span>
      <Tip tip={token.created_at} tap>
        <span className="mono muted small">Created {dateWords(token.created_at.slice(0, 10), today)}</span>
      </Tip>
      <Button disabled={busy} onClick={onRevoke} aria-label={`Revoke ${token.name}`} tip="It stops working at once">
        {revoking ? "Revoking..." : "Revoke"}
      </Button>
    </li>
  );
}

/** The new token, once: its secret, and the command that carries it for the client picked above. */
function Minted({ mcp, client, secret, onHide }: { mcp: McpAccess; client: Client; secret: string; onHide: () => void }) {
  const commands = tokenCommands(mcp, secret);
  // The form that asked for it is gone: the keyboard and a screen reader land on the result.
  const shown = useRef<HTMLLIElement>(null);
  useEffect(() => {
    shown.current?.focus();
  }, []);
  return (
    <li className="panel-body minted" data-testid="minted-token" tabIndex={-1} ref={shown}>
      <p className="warn-ink">Copy this token now: Cairn shows it only once.</p>
      {client === "claude" ? (
        <>
          <p>In a terminal on the machine where Claude Code runs, remove Cairn if you added it before, then add it again with the token:</p>
          <Copyable what="the command that removes Cairn" text={shellLine(commands.claudeRemove)} />
          <Copyable what="the Claude Code command with this token" text={shellLine(commands.claude)} />
        </>
      ) : client === "codex" ? (
        <>
          <p>Codex reads it from an environment variable you set where it runs:</p>
          <Copyable what="the Codex commands with this token" text={`${shellLine(commands.codexVariable)}\n${shellLine(commands.codex)}`} />
        </>
      ) : (
        <>
          <Copyable what="the token" text={secret} />
          <p>
            Send the header <Code>Authorization: Bearer</Code> and the token.
          </p>
        </>
      )}
      <span className="row">
        <Button onClick={onHide}>
          Hide token
        </Button>
        <span className="muted">Once hidden, it cannot be shown again.</span>
      </span>
    </li>
  );
}

type TokensMachine = ActorRefFrom<typeof agentTokens>;

/** The way to make a token: a button, then the name asked for. */
function MakeToken({ machine, refocus }: { machine: TokensMachine; refocus: boolean }) {
  const { context, minting, naming } = useSelector(machine, (snapshot) => ({
    context: snapshot.context,
    minting: snapshot.matches({ mint: "minting" }),
    naming: snapshot.matches({ mint: "naming" }) || snapshot.matches({ mint: "minting" }),
  }));
  return (
    <li className="panel-body minted">
      <p>A script or a client that cannot sign in through a browser can use a token. It acts as you until you revoke it.</p>
      {naming ? (
        <form
          className="form-col"
          onSubmit={(event) => {
            event.preventDefault();
            machine.send({ type: "make" });
          }}
        >
          <Input
            label="Token name"
            value={context.name}
            placeholder="Nightly report script"
            autoFocus
            maxLength={200}
            readOnly={minting}
            onChange={(event) => {
              machine.send({ type: "name", name: event.target.value });
            }}
          />
          <span className="row">
            <Button primary type="submit" disabled={context.name.trim() === "" || minting} tip="It is shown once">
              {minting ? "Making the token..." : "Make token"}
            </Button>
            <Button
              disabled={minting}
              onClick={() => {
                machine.send({ type: "cancel" });
              }}
            >
              Cancel
            </Button>
          </span>
        </form>
      ) : (
        <span className="row">
          <Button
            autoFocus={refocus}
            tip="For a script or a client that cannot sign in through a browser"
            onClick={() => {
              machine.send({ type: "open" });
            }}
          >
            New token
          </Button>
        </span>
      )}
    </li>
  );
}

/** The main flow, a client's browser sign-in, makes its token in another tab: read the list again on return. */
function useRefreshOnReturn(refresh: () => void) {
  useEffect(() => {
    const onVisible = () => {
      if (document.visibilityState === "visible") {
        refresh();
      }
    };
    document.addEventListener("visibilitychange", onVisible);
    // A window that stayed visible beside the one that signed the agent in gets the focus back.
    globalThis.addEventListener("focus", onVisible);
    return () => {
      document.removeEventListener("visibilitychange", onVisible);
      globalThis.removeEventListener("focus", onVisible);
    };
  }, [refresh]);
}

/** A revoke that went through is confirmed in a toast. */
function useRevokedToast(machine: TokensMachine) {
  const toasts = useSession().toasts;
  useEffect(() => {
    const subscription = machine.on("revoked", ({ name }) => {
      toasts.send({ type: "show", text: `Revoked ${name}`, tone: "confirm" });
    });
    return () => {
      subscription.unsubscribe();
    };
  }, [machine, toasts]);
}

/** Allowed agents: the tokens that are not revoked, the way to revoke one, and the way to make one. */
function AllowedAgents({ mcp, client, tokens, user }: { mcp: McpAccess; client: Client; tokens: AgentTokenHost; user: string }) {
  const machine = useActorRef(agentTokens, { input: { tokens } });
  const { problem, revoking, minted, version, showing } = useSelector(machine, (snapshot) => ({
    problem: snapshot.context.problem,
    revoking: snapshot.context.revoking,
    minted: snapshot.context.minted,
    version: snapshot.context.listVersion,
    showing: snapshot.matches({ mint: "shown" }),
  }));
  const { view, refresh } = useKeyedRead(TOKENS, { of: ["agent-tokens", user], freshness: [version], args: { tokens } });
  // Once a token has been hidden, the keyboard comes back to the button that makes the next one.
  const [hidden, setHidden] = useState(false);
  useRefreshOnReturn(refresh);
  useRevokedToast(machine);
  const rows = valueOf(view);
  return (
    <NodePanel title="Allowed agents" {...(rows === undefined ? {} : { count: rows.length })} testId="agent-tokens">
      {view.status === "failed" || view.status === "behind" ? (
        <li className="panel-body">
          <p className="callout callout-bad" role="alert">
            <Failed what="The list of allowed agents" error={view.error} retry>
              <Button onClick={refresh}>Try again</Button>
            </Failed>
          </p>
        </li>
      ) : rows === undefined ? (
        <li className="panel-body">
          <p className="muted" role="status">
            Reading the allowed agents...
          </p>
        </li>
      ) : null}
      {problem === undefined ? null : (
        <li className="panel-body">
          <p className="callout callout-bad" role="alert">
            {problem}
          </p>
        </li>
      )}
      {(rows ?? []).map((token) => (
        <TokenRow
          key={token.agent}
          token={token}
          busy={revoking !== undefined}
          revoking={revoking?.agent === token.agent}
          onRevoke={() => {
            machine.send({ type: "revoke", agent: token.agent, name: token.name });
          }}
        />
      ))}
      {showing && minted !== undefined ? (
        <Minted
          mcp={mcp}
          client={client}
          secret={minted.token.token}
          onHide={() => {
            machine.send({ type: "hide" });
            setHidden(true);
          }}
        />
      ) : (
        <MakeToken machine={machine} refocus={hidden} />
      )}
    </NodePanel>
  );
}

export function Agents() {
  const capabilities = useCapabilities();
  const mcp = capabilities.mcp ?? undefined;
  const { caller } = useCaller();
  const host = useSession().host;
  const [client, setClient] = useState<Client>("claude");
  if (mcp === undefined) {
    return (
      <section className="stack agents" aria-label="Connect an agent" data-testid="agents-screen">
        <h1>Connect an agent</h1>
        <p>This copy of Cairn runs in your browser, so no agent can connect to it. A Cairn server has an address agents connect to.</p>
      </section>
    );
  }
  const closed = agentSetup(mcp, client).closed;
  return (
    <section className="stack agents" aria-label="Connect an agent" data-testid="agents-screen">
      <h1>Connect an agent</h1>
      <p className="lead">Connect an AI agent, such as Claude Code or Codex, and it works in Cairn as you, with everything you can do here.</p>
      {closed ? (
        <p className="callout" data-testid="agents-closed">
          No sign-in for agents is set up on this Cairn: it signs people in only through a browser. Whoever runs it can turn one on.
        </p>
      ) : (
        <>
          <NodePanel title="Add it to your agent" testId="agent-clients">
            <li className="client-pick">
              <Segmented label="Client" value={client} options={CLIENTS} onChange={setClient} data-testid="client-control" />
            </li>
            <li className="panel-body" data-testid="client-steps" data-client={client}>
              <Steps mcp={mcp} client={client} />
            </li>
          </NodePanel>
          {mcp.tokens && host.agentTokens !== undefined && caller !== undefined ? <AllowedAgents key={caller.user} mcp={mcp} client={client} tokens={host.agentTokens} user={caller.user} /> : null}
        </>
      )}
    </section>
  );
}
