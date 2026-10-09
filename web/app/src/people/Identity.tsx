// H3, H1: who you are here. Your user, the identities you sign in with and the emails each
// provider verified, and the entities those emails name: you are each of them, so "mine"
// works with no link step. Two or more are duplicates of one person, and merging them is
// offered (H3). On the server, signing in with another provider while signed in links that
// identity to you (ARCHITECTURE, Auth: users and identities); it comes back here.
import { Link } from "react-router";

import type { Capabilities, Viewer } from "../data/host.ts";
import { useCapabilities, useDeployment, useSession, useViewer } from "../data/react.ts";
import { Segmented } from "../ui/kit.tsx";
import { THEME_CHOICES, useThemeChoice, type ThemeChoice } from "../ui/theme.ts";
import { byName, offeredMerge } from "./model.ts";

/** Where a sign-in with provider `name` starts, coming back to this screen (the tab keeps its host). */
export function signInHref(name: string): string {
  return `/api/auth/${name}/sign-in?return_to=${encodeURIComponent("/me")}`;
}

/** The providers a browser signs in with by being sent to them: OIDC on the server. */
export function linkable(capabilities: Capabilities): string[] {
  return capabilities.auth.filter((method) => method.kind === "oidc").map((method) => method.name);
}

function Identities({ viewer }: { viewer: Viewer }) {
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
          {viewer.identities.map((identity) => (
            <tr key={`${identity.provider}:${identity.subject}`} data-testid="identity" data-provider={identity.provider}>
              <td>{identity.provider}</td>
              <td className="mono">{identity.subject}</td>
              <td data-testid="identity-emails">{identity.verified_emails.join(", ") || <span className="muted small">None</span>}</td>
              <td className="muted small">{identity.linked_at}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function YourEntities({ viewer }: { viewer: Viewer }) {
  const deployment = useDeployment();
  const entities = byName((deployment?.entities ?? []).filter((entity) => viewer.entities.includes(entity.key)));
  const merge = offeredMerge(viewer.merge_offer);
  return (
    <section className="stack" aria-label="Your entities" data-testid="your-entities">
      <strong>You are</strong>
      {entities.length === 0 ? <span className="muted small" data-testid="no-entity">No entity holds a verified email of yours yet: add one to an entity on the entities screen, or link an identity whose email one holds.</span> : null}
      <ul className="row plain-list">
        {entities.map((entity) => (
          <li key={entity.key} className="badge" data-testid="your-entity" data-entity={entity.key}>
            {entity.name}
          </li>
        ))}
      </ul>
      {merge === undefined ? null : (
        <div className="callout stack" data-testid="merge-offer">
          <span>Your verified emails name {entities.length} entities: they are one person. "Mine" already covers them all; merging makes them one.</span>
          <Link to={`/entities?merge=${merge.survivor},${merge.merged}`}>Merge them</Link>
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

export function Identity() {
  const { viewer, failed } = useViewer();
  const { host } = useSession();
  const capabilities = useCapabilities();
  if (viewer === undefined) {
    return <p className={failed === undefined ? "muted small" : "callout callout-bad"}>{failed === undefined ? "Reading who you are..." : `Who you are could not be read: ${failed}`}</p>;
  }
  const providers = linkable(capabilities);
  return (
    <section className="stack" aria-label="You" data-testid="identity-screen">
      <span className="row">
        <h1>You</h1>
        <code className="mono small" data-testid="user">{viewer.user}</code>
        {host.kind === "browser" ? <span className="muted small">The demo's one local identity.</span> : null}
      </span>
      <Identities viewer={viewer} />
      <YourEntities viewer={viewer} />
      <ThemeControl />
      {providers.length === 0 ? null : (
        <span className="row" data-testid="link-identity">
          <span>Link another identity:</span>
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
