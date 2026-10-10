// The app frame every screen renders inside (design 3.1): a graphite rail with the screens,
// the workspace (the screen the router shows, its one scroller `.ws-body`), the inspector
// column on the right (INSPECTOR | ASSISTANT; a bottom sheet on a tablet), and the 28px
// strip with the sync chip. The frame is one `100dvh` grid with one scroller per
// region (app.css); below 720px it is one scrolling column.
import { Suspense } from "react";
import { Link, Outlet, useLocation } from "react-router";

import { FrameActionsContext, FrameStateContext } from "./frame.tsx";
import { InspectorColumn } from "./InspectorColumn.tsx";
import { LoadFailure } from "./LoadFailure.tsx";
import { SyncChip } from "./SyncChip.tsx";
import { Toast } from "./Toast.tsx";
import { useFrame, type Frame } from "./useFrame.ts";

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

/** The strip: the inspector's way back while it is collapsed on the left, the sync chip on the right. */
function Strip({ frame }: { frame: Frame }) {
  return (
    <footer className="strip">
      {frame.present && frame.collapsed ? (
        <button type="button" className="ghost strip-button" aria-expanded="false" onClick={frame.toggleCollapsed}>
          <kbd>]</kbd> Inspector
        </button>
      ) : null}
      <span className="spacer" />
      <SyncChip />
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
            <div className="ws-body">
              <LoadFailure screen={pathname}>
                <Suspense fallback={<p className="muted small">Loading...</p>}>
                  <Outlet />
                </Suspense>
              </LoadFailure>
            </div>
            <Toast />
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
