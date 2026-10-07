// The app shell every screen renders inside: the header (the host and its switch, whether
// the view is live), the version-skew banner, the screen, and the notices.
import { Link, Outlet } from "react-router";

import { switchedTo } from "../data/choose.ts";
import { useNotices, useSession, useSkew, useStreamStatus } from "../data/react.ts";
import { Badge, Button, Gate } from "../ui/kit.tsx";

function HostSwitch() {
  const { host } = useSession();
  const other = host.kind === "server" ? "browser" : "server";
  return (
    <span className="row">
      <Badge data-testid="host" data-status={host.kind}>{host.kind === "server" ? "Server" : "In-browser host"}</Badge>
      <a href={switchedTo(location, other)} className="muted">
        Switch to {other === "server" ? "the server" : "the in-browser host"}
      </a>
    </span>
  );
}

/** H6: live over the server's SSE stream (gated on its capability) or the in-browser notifier. */
function LiveStatus() {
  const status = useStreamStatus();
  const { host } = useSession();
  const tone = status === "live" ? "good" : status === "reconnecting" ? "warn" : "plain";
  const words = { idle: "Idle", connecting: "Connecting", live: "Live", reconnecting: "Reconnecting" }[status];
  return (
    <Gate when={(capabilities) => capabilities.sse || host.kind === "browser"} otherwise={<Badge tone="warn">Not live</Badge>}>
      <Badge tone={tone} data-testid="live" data-status={status}>
        {words}
      </Badge>
    </Gate>
  );
}

/** ARCHITECTURE, Web UI: version skew. Unsent edits are drafts, so the reload keeps them. */
function SkewBanner() {
  const skew = useSkew();
  if (skew === undefined) {
    return null;
  }
  return (
    <div className="banner banner-warn row" role="alert" data-testid="skew">
      <span>
        Cairn was updated (engine {skew.document}; this tab runs {skew.engine}). This tab has stopped deriving and
        saving until you reload. Your unsent edits are kept.
      </span>
      <Button
        primary
        onClick={() => {
          location.reload();
        }}
      >
        Reload
      </Button>
    </div>
  );
}

function Notices() {
  const notices = useNotices();
  const { notices: store } = useSession();
  return (
    <div className="notices" aria-live="polite">
      {notices.map((notice) => (
        <div key={notice.id} className={notice.tone === "problem" ? "panel callout-bad" : "panel"} data-testid="notice" data-tone={notice.tone}>
          <div className="row">
            <strong>{notice.title}</strong>
            <span className="shell-spacer" />
            <Button
              aria-label="Dismiss"
              onClick={() => {
                store.dismiss(notice.id);
              }}
            >
              ×
            </Button>
          </div>
          {notice.tone === "saved" && notice.lines.length === 0 ? (
            <span className="muted">Nothing became stale, short, overdue, or stalled.</span>
          ) : null}
          {notice.lines.map((line) => (
            <span key={`${line.journey}:${line.kind}`} className="muted" data-testid="consequence">
              {line.kind === "stalled" ? `${line.journey} is now stalled` : `Newly ${line.kind}: ${line.nodes.join(", ")}`}
            </span>
          ))}
        </div>
      ))}
    </div>
  );
}

export function Shell() {
  return (
    <>
      <header className="shell-header">
        <Link to="/" className="shell-brand">
          Cairn
        </Link>
        <nav className="row shell-nav" aria-label="Screens">
          <Link to="/">Journeys</Link>
          <Link to="/mine">Mine</Link>
          <Link to="/routes">Routes</Link>
          <Link to="/entities">Entities</Link>
          <Link to="/me">You</Link>
        </nav>
        <span className="shell-spacer" />
        <LiveStatus />
        <HostSwitch />
      </header>
      <SkewBanner />
      <main className="shell-main">
        <Outlet />
      </main>
      <Notices />
    </>
  );
}
