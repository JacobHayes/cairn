// The app frame every screen renders inside (design 3.1): a chrome rail with the screens,
// the workspace (the screen the router shows, its one scroller `.ws-body`), the inspector
// column on the right (INSPECTOR | ASSISTANT; a bottom sheet below 1100px; a rail when hidden). The sync dot sits
// beside the brand at the top of the rail. The frame is one `100dvh` grid with one scroller per
// region (app.css); below 720px it is one scrolling column.
import { Suspense } from "react";
import { Link, Outlet, useLocation } from "react-router";

import { pathTo, sectionOf } from "../address.ts";
import { useCapabilities } from "../data/react.ts";
import { HOME_PATH, indexPath } from "../journeys/address.ts";
import { ROUTES_INDEX_PATH } from "../routes/address.ts";
import { FrameActionsContext, FrameStateContext } from "./frame.tsx";
import { InspectorColumn } from "./InspectorColumn.tsx";
import { KeysHint } from "./KeysHint.tsx";
import { NamePrompt } from "./NamePrompt.tsx";
import { LoadFailure } from "./LoadFailure.tsx";
import { SyncChip } from "./SyncChip.tsx";
import { Toast } from "./Toast.tsx";
import { useFrame } from "./useFrame.ts";
import { Pending } from "../ui/Pending.tsx";

/** The screens, as links; the one the viewer is on is marked. */
function Rail({ pathname }: { pathname: string }) {
  const section = sectionOf(pathname);
  const offersMcp = useCapabilities().mcp != null;
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
        <span className="spacer" />
        {offersMcp ? (
          <Link to={pathTo("agents")} className="agents-link" title="Connect an AI agent such as Claude Code or Codex" {...(section === "agents" ? { "aria-current": "page" as const } : {})}>
            Connect an agent
          </Link>
        ) : null}
        <span className="rail-foot">
          <Link to={pathTo("me")} {...(section === "me" ? { "aria-current": "page" as const } : {})}>
            You
          </Link>
          <KeysHint />
        </span>
      </nav>
    </aside>
  );
}

export function Shell() {
  // The screen the router shows, for the browser tests to wait on after an in-page navigation.
  const { pathname, search } = useLocation();
  const frame = useFrame();
  return (
    <FrameActionsContext value={frame.actions}>
      <FrameStateContext value={frame.state}>
        <div className="frame" data-inspector={frame.folded ? "folded" : frame.open ? "on" : "off"} data-sheet={frame.snap}>
          <Rail pathname={pathname} />
          <NamePrompt />
          <main className="workspace" data-screen={`${pathname}${search}`}>
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
      </FrameStateContext>
    </FrameActionsContext>
  );
}
