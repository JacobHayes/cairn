// The app's entry: starts the tab's session (boot.ts), then renders the shell and its
// screens under a hash router, so the UI's addresses never collide with the API's paths the
// binary serves on the same origin and a static demo site needs no server rewrites.
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { HashRouter, Route, Routes } from "react-router";

import { ListScreen } from "./acting/ListScreen.tsx";
import { NextScreen } from "./acting/NextScreen.tsx";
import { TriageScreen } from "./acting/TriageScreen.tsx";
import { boot } from "./boot.ts";
import { LayoutsContext } from "./canvas/hooks.ts";
import { Layouts, LayoutWorker } from "./canvas/layouts.ts";
import { SessionContext } from "./data/react.ts";
import type { Session } from "./data/session.ts";
import { JourneyIndex } from "./screens/JourneyIndex.tsx";
import { JourneyPage } from "./screens/JourneyPage.tsx";
import { RouteCanvasPage } from "./screens/RouteCanvasPage.tsx";
import { Shell } from "./shell/Shell.tsx";
import "./ui/tokens.css";

function App({ session, layouts }: { session: Session; layouts: Layouts }) {
  return (
    <SessionContext value={session}>
      <LayoutsContext value={layouts}>
        <HashRouter>
          <Routes>
            <Route element={<Shell />}>
              <Route index element={<JourneyIndex />} />
              <Route path="journeys/:id" element={<JourneyPage />} />
              <Route path="journeys/:id/nodes/:key" element={<JourneyPage />} />
              <Route path="journeys/:id/next" element={<NextScreen />} />
              <Route path="journeys/:id/list" element={<ListScreen />} />
              <Route path="journeys/:id/triage" element={<TriageScreen />} />
              <Route path="routes/:id" element={<RouteCanvasPage />} />
            </Route>
          </Routes>
        </HashRouter>
      </LayoutsContext>
    </SessionContext>
  );
}

const container = document.getElementById("root");
if (container === null) {
  throw new Error("index.html has no #root");
}
const root = createRoot(container);
boot().then(
  (session) => {
    root.render(
      <StrictMode>
        <App session={session} layouts={new Layouts(new LayoutWorker())} />
      </StrictMode>,
    );
  },
  (thrown: unknown) => {
    root.render(
      <p className="callout callout-bad" role="alert">
        Cairn could not start: {thrown instanceof Error ? thrown.message : String(thrown)}
      </p>,
    );
  },
);
