// H3, H1: who you are here. Your identity (the one holding a verified email of yours, made at
// your first sign-in when none did), the logins you sign in with and the emails each provider
// verified. On the server, signing in with another provider while signed in links that login to
// you (ARCHITECTURE, Auth: users and logins); it comes back here.
import { Link } from "react-router";

import { pathTo } from "../address.ts";
import type { Capabilities, Caller } from "../data/host.ts";
import { useCapabilities, useCaller, useDeployment, useToday } from "../data/react.ts";
import { dateWords } from "../status/when.ts";
import { NodePanel } from "../acting/NodeItem.tsx";
import { Badge, Segmented } from "../ui/kit.tsx";
import { THEME_CHOICES, useThemeChoice, type ThemeChoice } from "../ui/theme.ts";
import { Failed } from "../ui/Failed.tsx";
import { identityLabel } from "./label.ts";

/** Where a sign-in with provider `name` starts, coming back to this screen (the tab keeps its host). */
export function signInHref(name: string): string {
  return `/api/auth/${name}/sign-in?return_to=${encodeURIComponent(pathTo("me"))}`;
}

/** The providers a browser signs in with by being sent to them: OIDC on the server. */
export function linkable(capabilities: Capabilities): string[] {
  return capabilities.auth.filter((method) => method.kind === "oidc").map((method) => method.name);
}

function Logins({ caller }: { caller: Caller }) {
  const today = useToday();
  return (
    <NodePanel title="Sign-in methods" count={caller.logins.length} testId="logins">
      {caller.logins.map((login) => {
        const linked = login.linked_at.slice(0, 10);
        return (
          <li key={`${login.provider}:${login.subject}`} className="you-row" data-testid="login" data-provider={login.provider}>
            <span className="row">
              <strong>{login.provider}</strong>
              <span className="mono">{login.subject}</span>
            </span>
            <span data-testid="login-emails">
              <span className="muted">Verified emails: </span>
              <span className="mono">{login.verified_emails.join(", ") || "none"}</span>
            </span>
            <span className="muted mono small" title={login.linked_at}>Linked {dateWords(linked, today)}</span>
          </li>
        );
      })}
    </NodePanel>
  );
}

function YourIdentities({ caller }: { caller: Caller }) {
  const deployment = useDeployment();
  // In key order: the first is the one you are shown as everywhere (the name prompt asks about it).
  const identities = (deployment?.identities ?? []).filter((identity) => caller.identities.includes(identity.key)).sort((one, other) => one.key.localeCompare(other.key));
  const several = identities.length > 1;
  return (
    <section className="stack" aria-label="Your identity" data-testid="your-identities">
      <NodePanel title={several ? "Your identities" : "Your identity"} count={identities.length}>
        {identities.map((identity, at) => (
          <li key={identity.key} className="you-row" data-testid="your-identity" data-identity={identity.key}>
            <span className="row">
              <Link to={pathTo("identity", { key: identity.key })} className={identity.name === undefined || identity.name === null ? "mono" : undefined}><strong>{identityLabel(identity)}</strong></Link>
              {several && at === 0 ? <Badge tone="good">Shown as</Badge> : null}
            </span>
            <span className="mono muted small">{(identity.name === undefined || identity.name === null ? (identity.emails ?? []).slice(1) : (identity.emails ?? [])).join(", ")}</span>
          </li>
        ))}
      </NodePanel>
      {several ? <span className="muted">Each holds a verified email of yours, so all of them are yours. You are shown as the first.</span> : null}
    </section>
  );
}

const THEME_WORDS: Record<ThemeChoice, string> = { system: "System", light: "Light", dark: "Dark" };

/** The theme: System follows the device, and the choice stays in this browser (ui/theme.ts). */
function ThemeControl() {
  const [choice, setChoice] = useThemeChoice();
  return (
    <section className="stack" aria-label="Theme" data-testid="theme">
      <span className="label">Theme</span>
      <Segmented
        label="Theme"
        value={choice}
        options={THEME_CHOICES.map((value) => ({ value, label: THEME_WORDS[value] }))}
        onChange={setChoice}
        data-testid="theme-control"
      />
      <span className="muted">System follows your device. The choice is kept in this browser only.</span>
    </section>
  );
}

/** What the app is built with that asks to be credited (the canvas's library: decisions/2026-10-09-the-canvas-attribution-is-hidden-and-xyflow-credited.md). */
function Licenses() {
  return (
    <details className="stack" data-testid="licenses">
      <summary>Licenses</summary>
      <p className="muted small">
        The graph is drawn with <a href="https://reactflow.dev">React Flow</a> by xyflow, under the MIT license. Its attribution badge is hidden here; xyflow asks organizations that hide it in commercial use to
        subscribe to <a href="https://reactflow.dev/pro">React Flow Pro</a> or <a href="https://github.com/sponsors/xyflow">sponsor the project</a>.
      </p>
    </details>
  );
}

export function You() {
  const { caller, failed } = useCaller();
  const capabilities = useCapabilities();
  if (caller === undefined) {
    return <p className={failed === undefined ? "muted small" : "callout callout-bad"}>{failed === undefined ? "Reading who you are..." : <Failed what="Who you are" error={failed} />}</p>;
  }
  const providers = linkable(capabilities);
  return (
    <section className="stack" aria-label="You" data-testid="you-screen">
      <h1>You</h1>
      <YourIdentities caller={caller} />
      <section className="stack" aria-label="Sign-in methods" data-testid="sign-in-methods">
        <Logins caller={caller} />
        {providers.length === 0 ? null : (
          <span className="row" data-testid="link-login">
            <span>Link another login:</span>
            {providers.map((name) => (
              <a key={name} className="button" href={signInHref(name)}>
                Sign in with {name}
              </a>
            ))}
          </span>
        )}
      </section>
      <ThemeControl />
      <Licenses />
    </section>
  );
}
