// The app frame every screen renders inside (design 3.1): a graphite rail with the screens,
// the workspace (the screen the router shows, its one scroller `.ws-body`), the inspector
// column on the right (INSPECTOR | ASSISTANT; a bottom sheet on a tablet), and the 28px
// strip with whether the view is live. The frame is one `100dvh` grid with one scroller per
// region (app.css); below 720px it is one scrolling column.
import { Link, Outlet, useLocation } from "react-router";

import { mayNotApply } from "../data/notices.ts";
import { useNotices, useSession, useSkew, useStreamStatus } from "../data/react.ts";
import { Badge, Button, Gate } from "../ui/kit.tsx";
import { FrameActionsContext, FrameStateContext } from "./frame.tsx";
import { InspectorColumn } from "./InspectorColumn.tsx";
import { useFrame, type Frame } from "./useFrame.ts";

/** The demo is the in-browser host: say so, since nothing done there is saved. */
function DemoBadge() {
  const { host } = useSession();
  if (host.kind !== "browser") {
    return null;
  }
  return (
    <Badge tone="superseded" title="Sample data in this tab. Nothing persists after a reload.">
      Demo
    </Badge>
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
    <div className="banner row" role="alert" data-testid="skew">
      <Badge tone="warn">Reload needed</Badge>
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

/** What just happened, rising at the workspace's bottom right above the strip (design 3.1). */
function Notices() {
  const notices = useNotices();
  const { notices: store } = useSession();
  return (
    <div className="toasts" aria-live="polite">
      {notices.map((notice) => (
        <div key={notice.id} className="toast-card stack" data-problem={notice.tone === "problem"} data-testid="notice" data-tone={notice.tone}>
          <div className="row">
            <strong>{notice.title}</strong>
            <span className="spacer" />
            <Button
              aria-label="Dismiss"
              onClick={() => {
                store.dismiss(notice.id);
              }}
            >
              ×
            </Button>
          </div>
          {notice.tone === "saved" && notice.lines.length === 0 ? <span className="small">Nothing became stale, short, overdue, or stalled.</span> : null}
          {notice.lines.map((line) => (
            <span key={`${line.journey}:${line.kind}`} className="small" data-testid="consequence" data-kind={line.kind}>
              {line.kind === "stalled"
                ? `${line.journey} is now stalled`
                : line.kind === "undecided"
                  ? `${mayNotApply(line.unanswered ?? [])}: ${line.nodes.join(", ")}`
                  : line.kind === "unanchored"
                    ? `No chain to the final milestone, so neither priority nor dates feel it (advisory): ${line.nodes.join(", ")}`
                    : `Newly ${line.kind}: ${line.nodes.join(", ")}`}
            </span>
          ))}
        </div>
      ))}
    </div>
  );
}

/** The screens, as links; the one the viewer is on is marked. */
function Rail({ pathname }: { pathname: string }) {
  const screens = [
    { to: "/mine", label: "Mine", on: pathname.startsWith("/mine") },
    { to: "/journeys", label: "Journeys", on: pathname === "/" || /^\/(journeys|new|proposals)(\/|$)/.test(pathname) },
    { to: "/library", label: "Library", on: /^\/(library|routes)(\/|$)/.test(pathname) },
    { to: "/entities", label: "Entities", on: pathname.startsWith("/entities") },
  ];
  return (
    <aside className="rail">
      <Link to="/" className="brand">
        Cairn
      </Link>
      <nav aria-label="Screens">
        {screens.map(({ to, label, on }) => (
          <Link key={to} to={to} {...(on ? { "aria-current": "page" as const } : {})}>
            {label}
          </Link>
        ))}
        <span className="spacer" />
        <Link to="/me" {...(pathname.startsWith("/me") ? { "aria-current": "page" as const } : {})}>
          You
        </Link>
      </nav>
    </aside>
  );
}

/** The strip: the inspector's way back while it is collapsed on the left, whether the view is live on the right. */
function Strip({ frame }: { frame: Frame }) {
  return (
    <footer className="strip">
      {frame.present && frame.collapsed ? (
        <button type="button" className="ghost strip-button" aria-expanded="false" onClick={frame.toggleCollapsed}>
          <kbd>]</kbd> Inspector
        </button>
      ) : null}
      <span className="spacer" />
      <LiveStatus />
      <DemoBadge />
    </footer>
  );
}

export function Shell() {
  // The screen the router shows, for the browser tests to wait on after an in-page navigation.
  const { pathname, search } = useLocation();
  const frame = useFrame();
  const mapOpen = new URLSearchParams(search).get("map") === "1";
  return (
    <FrameActionsContext value={frame.actions}>
      <FrameStateContext value={frame.state}>
        <div className="frame" data-inspector={frame.open ? "on" : "off"} data-map={mapOpen ? "open" : "closed"}>
          <Rail pathname={pathname} />
          <main className="workspace" data-screen={`${pathname}${search}`}>
            <SkewBanner />
            <div className="ws-body">
              <Outlet />
            </div>
            <Notices />
          </main>
          <InspectorColumn
            open={frame.open}
            tab={frame.state.tab}
            snap={frame.snap}
            assistantOffered={frame.dock !== undefined}
            onTab={frame.choose}
            onSnap={frame.setSnap}
            onCollapse={frame.toggleCollapsed}
            onClose={frame.close}
            inspectorRef={frame.setInspector}
            assistantRef={frame.setAssistant}
          />
          <Strip frame={frame} />
        </div>
      </FrameStateContext>
    </FrameActionsContext>
  );
}
