# Agents connect by what the capabilities say

## Question

A person needs to be told how to connect their agent (Claude Code, Codex, or any MCP client)
to the Cairn they use, worded for the sign-in that deployment has. Where does the page get
the address and the sign-in facts, where does it live, and how do agent tokens, which had no
screen at all, fit in? (Work: the Connect an agent page.)

## Call

- The server says it. The capabilities document's MCP entry is `null` where the host serves no
  MCP and otherwise holds the address (the configured public URL joined with the MCP path),
  how an agent signs in (the built-in OAuth server, the IAP relay, a tailnet, a dev user, or
  nothing), and whether agent tokens are accepted. The root works it out from the configured
  providers. The page reads neither provider kinds nor its own origin, so a deployment behind a
  proxy shows the address agents use, not the one the browser happens to see.
- The page is "Connect an agent", a rail link above the You line (a phone reaches it from the
  sync popover, beside You), present only where MCP is offered. The label says the job a
  first-time visitor has; "MCP" names the address panel only.
- One client at a time (a Segmented control, Claude Code first) in one panel, so the page leads
  with the command, not a manual or a separate address: the bare address is under Other agent.
  Rarely needed ways (a machine with no browser, project scope) sit behind one disclosure.
  Commands are checked against the installed clients and live in one module with the arguments
  as lists.
- Allowed agents and tokens are listed and made on this page, where tokens are accepted (they
  are not accepted without the built-in OAuth server). A token's secret is shown once, held
  only while shown, never stored. Revoke is one click with a toast: an allowed client signs in
  again in one step, and a token is cheap to make again.
- Making a token is refused (403) where no provider accepts tokens, instead of handing out a
  secret every request then refuses. Listing and revoking stay, so tokens stored while the
  server was configured can still be revoked through the API.
- The page promises only what the sign-in gives: the agent is recorded beside the person only
  for the built-in server and tokens, and only when no provider asked before it (the first with
  a credential names the actor) would name the person alone; Tailscale, IAP and dev name the
  person alone.
- Mint and revoke are auth calls, not domain writes, so they are not the edit manager's (a
  resend would make a second secret). One machine owned by the page runs them: there is no
  Cancel once a mint is out (its token would exist with its secret thrown away), revoking the
  token on show drops its secret, and the list is read again after any attempt whose outcome is
  uncertain.
- `cairn demo` gains the built-in OAuth server, listed first: the dev provider names every
  loopback request, and the first provider with a credential names the actor, so with dev
  first a token would act as the person alone. This lets the whole page be tried and tested.
  It does not show a first OAuth sign-in (dev answers before any challenge).

## Alternatives

- Build the address from the page's origin: wrong behind a proxy, and the server already knows
  the public URL.
- Put provider facts on the sign-in methods and let the page reason: duplicates the
  composition root's knowledge in the UI.
- Token management on the You page, or a fifth phone tab: a second path to a page that is
  already one tap from the popover, and You is about identity.
- All three clients stacked, or a `config.toml` block for Codex: longer pages and a second way
  to say what `codex mcp add` writes.

## What would change it

A client changing its flags; a deployment served at several origins; per-agent attribution for
Tailscale or IAP (separate auth work); a provider that lists agent tokens beside the built-in
server.
