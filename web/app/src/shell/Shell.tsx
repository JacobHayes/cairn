// The app frame every screen renders inside: a chrome rail with the screens,
// the workspace (the screen the router shows, its one scroller `.ws-body`), the inspector
// column on the right (INSPECTOR | ASSISTANT; a bottom sheet below 1100px; a rail when hidden). The sync dot sits
// beside the brand at the top of the rail. The frame is one `100dvh` grid with one scroller per
// region (app.css); below 720px it is one scrolling column.
import { Suspense } from "react";
import { Link, Outlet, useLocation } from "react-router";

import { pathTo, sectionOf } from "../address.ts";
import { HOME_PATH, indexPath } from "../journeys/address.ts";
import { ROUTES_INDEX_PATH } from "../routes/address.ts";
import { FrameActionsContext, FrameStateContext } from "./frame.tsx";
import { InspectorColumn } from "./InspectorColumn.tsx";
import { KeysHint } from "./KeysHint.tsx";
import { Tour, TourProvider, useTour } from "../tour/Tour.tsx";
import { NamePrompt, useNameAsk } from "./NamePrompt.tsx";
import { LoadFailure } from "./LoadFailure.tsx";
import { SyncChip } from "./SyncChip.tsx";
import { Toast } from "./Toast.tsx";
import { useFrame } from "./useFrame.ts";
import { PointedProvider } from "../canvas/pointed.tsx";
import { Pending } from "../ui/Pending.tsx";
import { Tip, TipLayer } from "../ui/Tip.tsx";

/** The screens, as links; the one the viewer is on is marked. */
function Rail({ pathname }: { pathname: string }) {
  const tour = useTour();
  const section = sectionOf(pathname);
  const screens = [
    { to: HOME_PATH, label: "Home", on: section === "home" },
    { to: indexPath(), label: "Journeys", on: section === "journeys" },
    { to: ROUTES_INDEX_PATH, label: "Routes", on: section === "routes" },
    { to: pathTo("identities"), label: "Identities", on: section === "identities" },
  ];
  return (
    <aside className="rail">
      <div className="rail-head">
        <Link to={HOME_PATH} className="brand">
          Cairn
        </Link>
        <SyncChip />
      </div>
      <nav aria-label="Screens">
        {screens.map(({ to, label, on }) => (
          <Link key={to} to={to} {...(on ? { "aria-current": "page" as const } : {})}>
            {label}
          </Link>
        ))}
        {tour?.railLabel === undefined ? null : (
          <button type="button" className="rail-tour" data-testid="rail-tour" onClick={tour.take}>
            {tour.railLabel}
          </button>
        )}
        <span className="spacer" />
        <span className="rail-foot">
          <Link to={pathTo("me")} {...(section === "me" ? { "aria-current": "page" as const } : {})}>
            You
          </Link>
          <KeysHint />
        </span>
        <Tip tip="Connect an AI agent such as Claude Code or Codex">
          <Link to={pathTo("agents")} className="agents-link" {...(section === "agents" ? { "aria-current": "page" as const } : {})}>
            Connect an agent
          </Link>
        </Tip>
      </nav>
    </aside>
  );
}

export function Shell() {
  // The screen the router shows, for the browser tests to wait on after an in-page navigation.
  const { pathname, search } = useLocation();
  const frame = useFrame();
  const ask = useNameAsk();
  return (
    <TipLayer>
    <PointedProvider>
    <FrameActionsContext value={frame.actions}>
      <FrameStateContext value={frame.state}>
        <TourProvider nameSettled={ask.settled}>
        <div className="frame" data-inspector={frame.folded ? "folded" : frame.open ? "on" : "off"} data-sheet={frame.snap}>
          <Rail pathname={pathname} />
          <NamePrompt ask={ask} />
          <main className="workspace" data-screen={`${pathname}${search}`}>
            <Tour />
            <div className="ws-body">
              <LoadFailure screen={pathname}>
                <Suspense fallback={<Pending>Loading...</Pending>}>
                  <Outlet />
                </Suspense>
              </LoadFailure>
            </div>
            <Toast />
          </main>
          <InspectorColumn
            present={frame.present}
            folded={frame.folded}
            tab={frame.state.tab}
            snap={frame.snap}
            tabs={frame.tabs}
            proposalsCount={frame.state.proposalsCount}
            onTab={frame.choose}
            onSnap={frame.setSnap}
            onToggle={frame.toggleFolded}
            inspectorRef={frame.setInspector}
            assistantRef={frame.setAssistant}
            proposalsRef={frame.setProposals}
          />
        </div>
        </TourProvider>
      </FrameStateContext>
    </FrameActionsContext>
    </PointedProvider>
    </TipLayer>
  );
}
