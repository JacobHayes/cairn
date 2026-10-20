// H3, H1: who you are here. Your user, the logins you sign in with and the emails each
// provider verified, and the identities those emails name: you are each of them, so "mine"
// works with no link step. Two or more are duplicates of one person, and merging them is
// offered (H3). On the server, signing in with another provider while signed in links that
// login to you (ARCHITECTURE, Auth: users and logins); it comes back here.
import { Link } from "react-router";

import { pathTo } from "../address.ts";
import type { Capabilities, Viewer } from "../data/host.ts";
import { useCapabilities, useDeployment, useSession, useUserName, useViewer } from "../data/react.ts";
import { Segmented } from "../ui/kit.tsx";
import { THEME_CHOICES, useThemeChoice, type ThemeChoice } from "../ui/theme.ts";
import { Failed } from "../ui/Failed.tsx";
import { byName, mergePath, offeredMerge } from "./model.ts";

/** Where a sign-in with provider `name` starts, coming back to this screen (the tab keeps its host). */
export function signInHref(name: string): string {
  return `/api/auth/${name}/sign-in?return_to=${encodeURIComponent(pathTo("me"))}`;
}

/** The providers a browser signs in with by being sent to them: OIDC on the server. */
export function linkable(capabilities: Capabilities): string[] {
  return capabilities.auth.filter((method) => method.kind === "oidc").map((method) => method.name);
}

function Logins({ viewer }: { viewer: Viewer }) {
  return (
    <div className="table-wrap">
      <table className="data">
        <thead>
          <tr>
            <th>Provider</th>
            <th>Account</th>
            <th>Verified emails</th>
            <th>Linked</th>
          </tr>
        </thead>
        <tbody>
          {viewer.logins.map((login) => (
            <tr key={`${login.provider}:${login.subject}`} data-testid="login" data-provider={login.provider}>
              <td>{login.provider}</td>
              <td className="mono">{login.subject}</td>
              <td className="mono" data-testid="login-emails">{login.verified_emails.join(", ") || <span className="muted small">None</span>}</td>
              <td className="muted small">{login.linked_at}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function YourIdentities({ viewer }: { viewer: Viewer }) {
  const deployment = useDeployment();
  const identities = byName((deployment?.identities ?? []).filter((identity) => viewer.identities.includes(identity.key)));
  const merge = offeredMerge(viewer.merge_offer);
  return (
    <section className="stack" aria-label="Your identities" data-testid="your-identities">
      <strong>You are</strong>
      {identities.length === 0 ? <span className="muted small" data-testid="no-identity">No identity holds a verified email of yours yet: add one to an identity on the identities screen, or link a login whose email one holds.</span> : null}
      <ul className="row plain-list">
        {identities.map((identity) => (
          <li key={identity.key} className="badge" data-testid="your-identity" data-identity={identity.key}>
            {identity.name}
          </li>
        ))}
      </ul>
      {merge === undefined ? null : (
        <div className="callout stack" data-testid="merge-offer">
          <span>Your verified emails name {identities.length} identities: they are one person. "Mine" already covers them all; merging makes them one.</span>
          <Link to={mergePath(merge.survivor, merge.merged)}>Merge them</Link>
        </div>
      )}
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
      <span className="muted small">System follows your device. The choice is kept in this browser only.</span>
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
  const { viewer, failed } = useViewer();
  const { host } = useSession();
  const capabilities = useCapabilities();
  const userName = useUserName();
  if (viewer === undefined) {
    return <p className={failed === undefined ? "muted small" : "callout callout-bad"}>{failed === undefined ? "Reading who you are..." : <Failed what="Who you are" error={failed} />}</p>;
  }
  const providers = linkable(capabilities);
  return (
    <section className="stack" aria-label="You" data-testid="you-screen">
      <span className="row">
        <h1>You</h1>
        <span className="muted small" data-testid="user">{userName(viewer.user)}</span>
        {host.kind === "browser" ? <span className="muted small">The demo's one local login.</span> : null}
      </span>
      <Logins viewer={viewer} />
      <YourIdentities viewer={viewer} />
      <ThemeControl />
      <Licenses />
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
  );
}
